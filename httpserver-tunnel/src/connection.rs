// Phase 7.1 Tunnel Connection Management
// Handles WebSocket connections, auto-reconnection, and connection state

use crate::{TunnelError, TunnelResult};
use crate::auth::{TunnelAuthenticator, TunnelCredentials};
use crate::config::{TunnelEndpoint, ReconnectionConfig};
use crate::status::{ConnectionHealth, TunnelMetrics};
use crate::protocol::TunnelMessage;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, watch};
use tokio::time::{interval, sleep};
use tokio_tungstenite::{connect_async_tls_with_config, tungstenite::Message};
use tracing;
use url::Url;

/// Connection state
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Authenticating,
    Authenticated,
    Reconnecting,
    Failed(String),
}

/// Reconnection strategy
#[derive(Debug, Clone)]
pub enum ReconnectionStrategy {
    Exponential {
        initial_delay: Duration,
        max_delay: Duration,
        multiplier: f64,
        jitter_factor: f64,
    },
    Fixed {
        delay: Duration,
    },
    Linear {
        initial_delay: Duration,
        increment: Duration,
        max_delay: Duration,
    },
}

impl ReconnectionStrategy {
    /// Calculate next retry delay
    pub fn next_delay(&self, attempt: u32) -> Duration {
        match self {
            ReconnectionStrategy::Exponential { initial_delay, max_delay, multiplier, jitter_factor } => {
                let base_delay = initial_delay.as_secs_f64() * multiplier.powi(attempt as i32);
                let max_delay_secs = max_delay.as_secs_f64();
                let delay_secs = base_delay.min(max_delay_secs);
                
                // Add jitter
                let jitter = delay_secs * jitter_factor * (rand::random::<f64>() - 0.5);
                let final_delay = (delay_secs + jitter).max(0.0);
                
                Duration::from_secs_f64(final_delay)
            },
            ReconnectionStrategy::Fixed { delay } => *delay,
            ReconnectionStrategy::Linear { initial_delay, increment, max_delay } => {
                let delay_secs = initial_delay.as_secs() + (increment.as_secs() * attempt as u64);
                Duration::from_secs(delay_secs.min(max_delay.as_secs()))
            },
        }
    }
}

impl From<ReconnectionConfig> for ReconnectionStrategy {
    fn from(config: ReconnectionConfig) -> Self {
        ReconnectionStrategy::Exponential {
            initial_delay: Duration::from_secs(config.initial_delay),
            max_delay: Duration::from_secs(config.max_delay),
            multiplier: config.backoff_multiplier,
            jitter_factor: config.jitter_factor,
        }
    }
}

/// Tunnel connection manager
pub struct TunnelConnection {
    endpoint: TunnelEndpoint,
    authenticator: Arc<TunnelAuthenticator>,
    state: Arc<RwLock<ConnectionState>>,
    reconnection_strategy: ReconnectionStrategy,
    reconnection_config: ReconnectionConfig,
    
    // Communication channels
    message_sender: Option<mpsc::UnboundedSender<TunnelMessage>>,
    status_sender: watch::Sender<ConnectionState>,
    status_receiver: watch::Receiver<ConnectionState>,
    
    // Connection metrics
    metrics: Arc<RwLock<TunnelMetrics>>,
    connection_start: Option<Instant>,
    retry_count: u32,
    
    // Connection info
    public_url: Arc<RwLock<Option<String>>>,
    tunnel_id: Arc<RwLock<Option<String>>>,
    session_id: Arc<RwLock<Option<String>>>,
}

impl TunnelConnection {
    /// Create new tunnel connection
    pub fn new(
        endpoint: TunnelEndpoint,
        authenticator: Arc<TunnelAuthenticator>,
        reconnection_config: ReconnectionConfig,
    ) -> Self {
        let (status_sender, status_receiver) = watch::channel(ConnectionState::Disconnected);
        let reconnection_strategy = ReconnectionStrategy::from(reconnection_config.clone());
        
        Self {
            endpoint,
            authenticator,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            reconnection_strategy,
            reconnection_config,
            message_sender: None,
            status_sender,
            status_receiver,
            metrics: Arc::new(RwLock::new(TunnelMetrics::new())),
            connection_start: None,
            retry_count: 0,
            public_url: Arc::new(RwLock::new(None)),
            tunnel_id: Arc::new(RwLock::new(None)),
            session_id: Arc::new(RwLock::new(None)),
        }
    }

    /// Start tunnel connection with auto-reconnection
    pub async fn start(&mut self, local_port: u16) -> TunnelResult<()> {
        self.set_state(ConnectionState::Connecting).await;
        
        loop {
            match self.connect_once(local_port).await {
                Ok(()) => {
                    // Connection successful, reset retry count
                    self.retry_count = 0;
                    tracing::info!("Tunnel connection established successfully");
                    
                    // Connection is maintained in connect_once until it fails
                    // If we reach here, the connection was lost
                    if !self.reconnection_config.enabled {
                        tracing::info!("Auto-reconnection disabled, stopping");
                        break;
                    }
                    
                    self.set_state(ConnectionState::Reconnecting).await;
                }
                Err(e) => {
                    tracing::error!(error = %e, "Tunnel connection failed");
                    self.set_state(ConnectionState::Failed(e.to_string())).await;
                    
                    if !self.reconnection_config.enabled {
                        return Err(e);
                    }
                    
                    // Check retry limit
                    if self.reconnection_config.max_attempts > 0 && 
                       self.retry_count >= self.reconnection_config.max_attempts {
                        tracing::error!(
                            attempts = self.retry_count,
                            max_attempts = self.reconnection_config.max_attempts,
                            "Maximum retry attempts reached"
                        );
                        return Err(TunnelError::ConnectionFailed("Max retry attempts exceeded".to_string()));
                    }
                }
            }
            
            // Calculate retry delay
            let delay = self.reconnection_strategy.next_delay(self.retry_count);
            self.retry_count += 1;
            
            tracing::info!(
                delay_secs = delay.as_secs(),
                attempt = self.retry_count,
                "Retrying tunnel connection"
            );
            
            sleep(delay).await;
        }
        
        Ok(())
    }

    /// Attempt single connection
    async fn connect_once(&mut self, _local_port: u16) -> TunnelResult<()> {
        self.connection_start = Some(Instant::now());
        
        // Parse WebSocket URL
        let ws_url = Url::parse(&self.endpoint.server_url)
            .map_err(|e| TunnelError::InvalidConfig(format!("Invalid server URL: {}", e)))?;
        
        tracing::info!(url = %ws_url, "Connecting to tunnel server");

        // Get authentication credentials
        let credentials = self.authenticator.get_credentials().await?;
        
        // Set up TLS configuration if needed
        let tls_config = if ws_url.scheme() == "wss" {
            Some(self.create_tls_config().await?)
        } else {
            None
        };        // Connect to WebSocket
        let (ws_stream, _) = if let Some(tls_config) = tls_config {
            // For tokio-tungstenite, we need to use their Connector type
            use tokio_tungstenite::Connector;
            let connector = Connector::Rustls(Arc::new(tls_config));
            connect_async_tls_with_config(&ws_url, None, false, Some(connector))
                .await
                .map_err(|e| TunnelError::ConnectionFailed(format!("WebSocket TLS connection failed: {}", e)))?
        } else {
            tokio_tungstenite::connect_async(&ws_url)
                .await
                .map_err(|e| TunnelError::ConnectionFailed(format!("WebSocket connection failed: {}", e)))?
        };

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        
        self.set_state(ConnectionState::Connected).await;
        
        // Authenticate
        self.set_state(ConnectionState::Authenticating).await;
        let auth_success = self.authenticate(&mut ws_sender, &mut ws_receiver, &credentials).await?;
        
        if !auth_success {
            return Err(TunnelError::AuthenticationFailed("Server rejected authentication".to_string()));
        }
          self.set_state(ConnectionState::Authenticated).await;
        let public_url = self.get_public_url().await.unwrap_or_default();
        tracing::info!(
            public_url = %public_url,
            "Tunnel established successfully"
        );
        
        // Start message handling
        let (message_tx, mut message_rx) = mpsc::unbounded_channel::<TunnelMessage>();
        self.message_sender = Some(message_tx);
        
        // Keep-alive task
        let keepalive_interval_secs = self.endpoint.keepalive_interval;
        let mut keepalive_timer = interval(Duration::from_secs(keepalive_interval_secs));
        
        // Main message loop
        loop {
            tokio::select! {
                // Handle incoming WebSocket messages
                msg_result = ws_receiver.next() => {
                    match msg_result {
                        Some(Ok(msg)) => {
                            if let Err(e) = self.handle_websocket_message(msg).await {
                                tracing::error!(error = %e, "Error handling WebSocket message");
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            tracing::error!(error = %e, "WebSocket receive error");
                            break;
                        }
                        None => {
                            tracing::info!("WebSocket connection closed by server");
                            break;
                        }
                    }
                }
                
                // Handle outgoing messages
                Some(tunnel_msg) = message_rx.recv() => {
                    let ws_msg = Message::Text(serde_json::to_string(&tunnel_msg).unwrap());
                    if let Err(e) = ws_sender.send(ws_msg).await {
                        tracing::error!(error = %e, "Failed to send WebSocket message");
                        break;
                    }
                }
                
                // Send keep-alive ping
                _ = keepalive_timer.tick() => {
                    let ping_msg = TunnelMessage::Ping {
                        timestamp: chrono::Utc::now().timestamp() as u64,
                    };
                    let ws_msg = Message::Text(serde_json::to_string(&ping_msg).unwrap());
                    if let Err(e) = ws_sender.send(ws_msg).await {
                        tracing::error!(error = %e, "Failed to send keep-alive ping");
                        break;
                    }
                }
            }
        }
        
        self.set_state(ConnectionState::Disconnected).await;
        Ok(())
    }

    /// Create TLS configuration for secure WebSocket connection
    async fn create_tls_config(&self) -> TunnelResult<tokio_rustls::rustls::ClientConfig> {        let config = tokio_rustls::rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(self.get_root_certificates()?)
            .with_no_client_auth();

        // Add client certificate if configured
        if self.authenticator.has_client_certificate() {            if let (Some(_cert_file), Some(_key_file)) = (
                self.authenticator.get_cert_file(),
                self.authenticator.get_key_file(),
            ) {
                // Load client certificate and key (simplified for now)
                tracing::info!("Loading client certificate for mutual TLS");
                // TODO: Implement client certificate loading
            }
        }

        Ok(config)
    }

    /// Get root certificates for TLS verification
    fn get_root_certificates(&self) -> TunnelResult<tokio_rustls::rustls::RootCertStore> {
        let mut root_store = tokio_rustls::rustls::RootCertStore::empty();
          // Use system root certificates
        for cert in webpki_roots::TLS_SERVER_ROOTS.iter() {
            root_store.add_trust_anchors(std::iter::once(
                tokio_rustls::rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                    cert.subject,
                    cert.spki,
                    cert.name_constraints,
                )
            ));
        }
        
        Ok(root_store)
    }

    /// Handle authentication with tunnel server
    async fn authenticate(
        &self,
        ws_sender: &mut futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>,
        ws_receiver: &mut futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
        credentials: &TunnelCredentials,
    ) -> TunnelResult<bool> {        // Create authentication message
        let auth_header = credentials.headers.get("Authorization")
            .ok_or_else(|| TunnelError::AuthenticationFailed("No authorization header".to_string()))?;
        
        // Extract token from "Bearer token" format
        let token = if auth_header.starts_with("Bearer ") {
            auth_header.trim_start_matches("Bearer ").to_string()
        } else {
            auth_header.clone()
        };
        
        let auth_msg = TunnelMessage::Auth {
            token,
            subdomain: self.endpoint.subdomain.clone(),
            protocol_version: self.endpoint.protocol_version.clone(),
        };

        // Send authentication
        let ws_msg = Message::Text(serde_json::to_string(&auth_msg)
            .map_err(|e| TunnelError::ProtocolError(format!("Failed to serialize auth message: {}", e)))?);
        
        ws_sender.send(ws_msg).await
            .map_err(|e| TunnelError::ConnectionFailed(format!("Failed to send auth message: {}", e)))?;        // Wait for authentication response
        match tokio::time::timeout(Duration::from_secs(30), ws_receiver.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                match serde_json::from_str::<TunnelMessage>(&text) {
                    Ok(TunnelMessage::AuthResponse { success, assigned_subdomain, error }) => {
                        if success {
                            tracing::info!("Authentication successful");
                            if let Some(subdomain) = assigned_subdomain {
                                // Construct public URL from subdomain and base domain
                                // This should ideally come from server config but we'll construct it
                                let public_url = format!("http://{}.httpserver.io", subdomain);
                                *self.public_url.write().await = Some(public_url);
                            }
                            Ok(true)
                        } else {
                            let error_msg = error.unwrap_or_else(|| "Unknown error".to_string());
                            tracing::error!(error = %error_msg, "Authentication failed");
                            Ok(false)
                        }
                    }                    Ok(TunnelMessage::Error { code, message }) => {
                        Err(TunnelError::AuthenticationFailed(format!("Auth error {}: {}", code, message)))
                    }
                    Ok(_) => {
                        Err(TunnelError::ProtocolError("Unexpected message during authentication".to_string()))
                    }
                    Err(e) => {
                        Err(TunnelError::ProtocolError(format!("Failed to parse auth response: {}", e)))
                    }
                }
            }
            Ok(Some(Ok(_))) => {
                Err(TunnelError::ProtocolError("Unexpected message type during authentication".to_string()))
            }
            Ok(Some(Err(e))) => {
                Err(TunnelError::ConnectionFailed(format!("WebSocket error during auth: {}", e)))
            }
            Ok(None) => {
                Err(TunnelError::ConnectionFailed("Connection closed during authentication".to_string()))
            }
            Err(_) => {
                Err(TunnelError::ConnectionFailed("Authentication timeout".to_string()))
            }
        }
    }    /// Handle incoming WebSocket message
    async fn handle_websocket_message(&self, message: Message) -> TunnelResult<()> {
        match message {
            Message::Text(text) => {
                match serde_json::from_str::<TunnelMessage>(&text) {
                    Ok(tunnel_msg) => {
                        self.process_tunnel_message(tunnel_msg).await
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, text = %text, "Failed to parse tunnel message");
                        Ok(())
                    }
                }
            }
            Message::Binary(_) => {
                tracing::warn!("Received unexpected binary message");
                Ok(())
            }            Message::Ping(_data) => {
                // WebSocket ping handled automatically by tungstenite
                tracing::debug!("Received WebSocket ping");
                Ok(())
            }
            Message::Pong(_) => {
                tracing::debug!("Received WebSocket pong");
                Ok(())
            }
            Message::Close(_) => {
                tracing::info!("Received WebSocket close message");
                Err(TunnelError::ConnectionFailed("WebSocket closed by server".to_string()))
            }
            Message::Frame(_) => {
                tracing::debug!("Received WebSocket frame");
                Ok(())
            }
        }
    }

    /// Process tunnel protocol message
    async fn process_tunnel_message(&self, message: TunnelMessage) -> TunnelResult<()> {
        match message {            TunnelMessage::HttpRequest { id, method, path, headers: _, body: _, client_ip: _ } => {
                // TODO: Forward HTTP request to local server
                tracing::debug!(id = %id, method = %method, path = %path, "Received HTTP request");
                Ok(())
            }
            TunnelMessage::Pong { timestamp } => {
                tracing::debug!(timestamp = timestamp, "Received pong");
                self.update_metrics_on_pong().await;
                Ok(())
            }            TunnelMessage::Status { connections, bytes_sent, bytes_received } => {
                tracing::debug!(
                    connections = connections,
                    bytes_sent = bytes_sent,
                    bytes_received = bytes_received,
                    "Received status update"
                );
                self.update_server_metrics(connections, bytes_sent, bytes_received).await;
                Ok(())
            }
            TunnelMessage::Error { code, message } => {
                tracing::error!(code = %code, message = %message, "Received error from server");
                Err(TunnelError::ProtocolError(format!("Server error {}: {}", code, message)))
            }
            _ => {
                tracing::warn!("Received unexpected tunnel message");
                Ok(())
            }
        }
    }

    /// Update connection state
    async fn set_state(&self, state: ConnectionState) {
        *self.state.write().await = state.clone();
        let _ = self.status_sender.send(state);
    }

    /// Get current connection state
    pub async fn get_state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    /// Get public URL if tunnel is established
    pub async fn get_public_url(&self) -> Option<String> {
        self.public_url.read().await.clone()
    }

    /// Get tunnel ID if established
    pub async fn get_tunnel_id(&self) -> Option<String> {
        self.tunnel_id.read().await.clone()
    }

    /// Get connection health status
    pub async fn get_health(&self) -> ConnectionHealth {
        let state = self.get_state().await;
        let uptime = self.connection_start
            .map(|start| start.elapsed())
            .unwrap_or_default();        ConnectionHealth {
            state,
            uptime,
            retry_count: self.retry_count,
            last_error: None, // TODO: Track last error
            health_score: 0, // TODO: Calculate health score
            last_ping: None, // TODO: Track last ping
            avg_ping_latency: None, // TODO: Calculate average latency
        }
    }

    /// Update metrics on pong received
    async fn update_metrics_on_pong(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.last_ping_time = Some(chrono::Utc::now());
        metrics.total_pings += 1;
    }    /// Update server metrics
    async fn update_server_metrics(&self, connections: u32, bytes_sent: u64, bytes_received: u64) {
        let mut metrics = self.metrics.write().await;
        // Map new field names to existing metrics structure
        metrics.connected_clients = connections;
        metrics.bytes_transferred = bytes_sent + bytes_received; // Combined for now
        // Keep server_uptime as is since we don't have uptime in new protocol
    }

    /// Get connection metrics
    pub async fn get_metrics(&self) -> TunnelMetrics {
        self.metrics.read().await.clone()
    }

    /// Subscribe to status updates
    pub fn subscribe_status(&self) -> watch::Receiver<ConnectionState> {
        self.status_receiver.clone()
    }
}
