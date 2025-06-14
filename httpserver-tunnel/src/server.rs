// Phase 7.2 Tunnel Server - Public HTTP Server Integration
// Tunnel server that accepts WebSocket connections from tunnel clients and routes public traffic

use crate::{TunnelError, config::{TunnelServerConfig, SubdomainStrategy}};
use crate::protocol::{TunnelMessage, TunnelProtocol};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, broadcast, mpsc, oneshot};
use futures_util::{SinkExt, StreamExt};
use uuid::Uuid;
use axum::{
    extract::{State, WebSocketUpgrade, ConnectInfo},
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{any, get},
    Router,
};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tracing::{info, warn, error, debug};
use rand::{Rng, thread_rng};
use rand::distributions::Alphanumeric;

/// Tunnel server result type
pub type ServerResult<T> = Result<T, TunnelError>;

/// Pending HTTP request waiting for response
#[derive(Debug)]
pub struct PendingRequest {
    pub id: String,
    pub response_sender: oneshot::Sender<TunnelMessage>,
    pub created_at: std::time::Instant,
}

/// Active tunnel connection
#[derive(Debug, Clone)]
pub struct ActiveTunnel {
    pub id: String,
    pub subdomain: String,
    pub client_ip: String,
    pub authenticated: bool,
    pub connected_at: std::time::SystemTime,
    pub request_sender: mpsc::Sender<TunnelMessage>,
}

/// Tunnel server state
#[derive(Debug)]
pub struct TunnelServerState {
    pub config: TunnelServerConfig,
    pub active_tunnels: RwLock<HashMap<String, ActiveTunnel>>,
    pub subdomain_mapping: RwLock<HashMap<String, String>>, // subdomain -> tunnel_id
    pub pending_requests: RwLock<HashMap<String, PendingRequest>>, // request_id -> pending_request
    pub protocol: TunnelProtocol,
    pub shutdown_sender: broadcast::Sender<()>,
}

/// Main tunnel server
pub struct TunnelServer {
    config: TunnelServerConfig,
    state: Arc<TunnelServerState>,
}

impl TunnelServer {    /// Create new tunnel server
    pub fn new(config: TunnelServerConfig) -> ServerResult<Self> {
        let (shutdown_sender, _) = broadcast::channel(100);
          let state = Arc::new(TunnelServerState {
            config: config.clone(),
            active_tunnels: RwLock::new(HashMap::new()),
            subdomain_mapping: RwLock::new(HashMap::new()),
            pending_requests: RwLock::new(HashMap::new()),
            protocol: TunnelProtocol::new(),
            shutdown_sender,
        });

        Ok(Self { config, state })
    }    /// Start the tunnel server
    pub async fn start(&self) -> ServerResult<()> {
        if !self.config.enabled {
            info!("Tunnel server is disabled");
            return Ok(());
        }

        info!("Starting tunnel server on port {}", self.config.tunnel_port);

        // Create the main router for public traffic
        let public_router = self.create_public_router();
        
        // Create the tunnel WebSocket endpoint
        let tunnel_router = self.create_tunnel_router();

        // Start cleanup task for expired requests
        let state_for_cleanup = self.state.clone();
        tokio::spawn(async move {
            Self::cleanup_expired_requests(state_for_cleanup).await;
        });        // Start both servers concurrently
        let public_addr = SocketAddr::new(
            self.config.network.public_bind_address.parse()
                .map_err(|e| TunnelError::ConfigError(format!("Invalid public bind address: {}", e)))?,
            self.config.public_port
        );
        let tunnel_addr = SocketAddr::new(
            self.config.network.bind_address.parse()
                .map_err(|e| TunnelError::ConfigError(format!("Invalid tunnel bind address: {}", e)))?,
            self.config.tunnel_port
        );

        info!("Tunnel server listening for public traffic on {}", public_addr);
        info!("Tunnel server listening for tunnel connections on {}", tunnel_addr);

        // Run both servers
        let public_server = axum::serve(
            tokio::net::TcpListener::bind(public_addr).await
                .map_err(|e| TunnelError::NetworkError(format!("Failed to bind public address: {}", e)))?,
            public_router,
        );

        let tunnel_server = axum::serve(
            tokio::net::TcpListener::bind(tunnel_addr).await
                .map_err(|e| TunnelError::NetworkError(format!("Failed to bind tunnel address: {}", e)))?,
            tunnel_router.into_make_service_with_connect_info::<SocketAddr>(),
        );

        // Run both servers concurrently
        tokio::try_join!(
            async { public_server.await.map_err(|e| TunnelError::NetworkError(e.to_string())) },
            async { tunnel_server.await.map_err(|e| TunnelError::NetworkError(e.to_string())) }
        )?;

        Ok(())
    }    /// Create router for public traffic (subdomain routing)
    fn create_public_router(&self) -> Router {
        Router::new()
            .route("/*path", any(Self::handle_public_request))
            .layer(
                ServiceBuilder::new()
                    .layer(CorsLayer::permissive())
                    .into_inner(),
            )
            .with_state(self.state.clone())
    }

    /// Create router for tunnel WebSocket connections
    fn create_tunnel_router(&self) -> Router {
        Router::new()
            .route("/connect", get(Self::handle_tunnel_connection))
            .route("/health", get(Self::handle_health_check))
            .with_state(self.state.clone())
    }    /// Handle incoming public HTTP requests
    async fn handle_public_request(
        State(state): State<Arc<TunnelServerState>>,
        method: Method,
        uri: Uri,
        headers: HeaderMap,
        body: axum::body::Bytes,
    ) -> Response {
        // Extract subdomain from Host header
        let host = match headers.get("host") {
            Some(host) => host.to_str().unwrap_or(""),
            None => {
                return (StatusCode::BAD_REQUEST, "Missing Host header").into_response();
            }
        };

        let subdomain = match Self::extract_subdomain(host, &state.config.base_domain) {
            Some(sub) => sub,
            None => {
                return (StatusCode::NOT_FOUND, "Invalid subdomain").into_response();
            }
        };

        // Find the tunnel for this subdomain
        let tunnel_id = {
            let mapping = state.subdomain_mapping.read().await;
            match mapping.get(&subdomain) {
                Some(id) => id.clone(),
                None => {
                    return (StatusCode::NOT_FOUND, "Tunnel not found").into_response();
                }
            }
        };

        // Get the tunnel connection
        let tunnel = {
            let tunnels = state.active_tunnels.read().await;
            match tunnels.get(&tunnel_id) {
                Some(tunnel) => tunnel.clone(),
                None => {
                    return (StatusCode::BAD_GATEWAY, "Tunnel disconnected").into_response();
                }
            }
        };

        // Generate unique request ID for correlation
        let request_id = Uuid::new_v4().to_string();

        // Create channel for response
        let (response_sender, response_receiver) = oneshot::channel();

        // Store pending request
        {
            let mut pending = state.pending_requests.write().await;
            pending.insert(request_id.clone(), PendingRequest {
                id: request_id.clone(),
                response_sender,
                created_at: std::time::Instant::now(),
            });
        }

        // Forward the request through the tunnel
        let request_message = TunnelMessage::HttpRequest {
            id: request_id.clone(),
            method: method.as_str().to_string(),
            path: uri.path_and_query().map(|pq| pq.as_str()).unwrap_or(uri.path()).to_string(),
            headers: Self::headers_to_hashmap(&headers),
            body: if body.is_empty() { None } else { Some(body.to_vec()) },
            client_ip: "0.0.0.0".to_string(), // TODO: Extract real client IP
        };

        // Send request to tunnel client
        if let Err(e) = tunnel.request_sender.send(request_message).await {
            error!("Failed to send request to tunnel: {}", e);
            // Clean up pending request
            state.pending_requests.write().await.remove(&request_id);
            return (StatusCode::BAD_GATEWAY, "Tunnel communication error").into_response();
        }

        // Wait for response with timeout
        let response_timeout = Duration::from_secs(30);
        match tokio::time::timeout(response_timeout, response_receiver).await {
            Ok(Ok(tunnel_response)) => {
                // Convert tunnel response to HTTP response
                Self::tunnel_response_to_http(tunnel_response)
            }
            Ok(Err(_)) => {
                // Channel closed
                (StatusCode::BAD_GATEWAY, "Tunnel closed during request").into_response()
            }
            Err(_) => {
                // Timeout
                state.pending_requests.write().await.remove(&request_id);
                (StatusCode::GATEWAY_TIMEOUT, "Request timeout").into_response()
            }
        }
    }    /// Handle tunnel WebSocket connection establishment
    async fn handle_tunnel_connection(
        ws: WebSocketUpgrade,
        State(state): State<Arc<TunnelServerState>>,
        ConnectInfo(addr): ConnectInfo<SocketAddr>,
    ) -> Response {
        info!("New tunnel connection attempt from {}", addr);
        ws.on_upgrade(move |socket| Self::handle_tunnel_websocket(socket, state, addr.to_string()))
    }    /// Handle tunnel WebSocket communication
    async fn handle_tunnel_websocket(
        socket: axum::extract::ws::WebSocket,
        state: Arc<TunnelServerState>,
        client_ip: String,
    ) {        let (sender, mut receiver) = socket.split();
        let tunnel_id = Uuid::new_v4().to_string();
        
        info!("New tunnel connection: {} from {}", tunnel_id, client_ip);

        // Create channel for sending requests to this tunnel
        let (_request_sender, mut _request_receiver) = mpsc::channel::<TunnelMessage>(100);

        // Handle incoming messages from tunnel client
        let state_clone = state.clone();
        let tunnel_id_clone = tunnel_id.clone();
        let sender_handle = Arc::new(tokio::sync::Mutex::new(sender));
        
        // Handle incoming WebSocket messages
        let sender_for_incoming = sender_handle.clone();
        let incoming_task = tokio::spawn(async move {
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(axum::extract::ws::Message::Text(text)) => {
                        if let Ok(tunnel_msg) = TunnelProtocol::deserialize_message(text.as_bytes()) {
                            Self::handle_tunnel_message(
                                tunnel_msg,
                                &tunnel_id_clone,
                                &state_clone,
                                &sender_for_incoming,
                            ).await;
                        }
                    }
                    Ok(axum::extract::ws::Message::Binary(data)) => {
                        if let Ok(tunnel_msg) = TunnelProtocol::deserialize_message(&data) {
                            Self::handle_tunnel_message(
                                tunnel_msg,
                                &tunnel_id_clone,
                                &state_clone,
                                &sender_for_incoming,
                            ).await;
                        }
                    }
                    Ok(axum::extract::ws::Message::Close(_)) => {
                        info!("Tunnel {} disconnected", tunnel_id_clone);
                        break;
                    }
                    Err(e) => {
                        error!("Tunnel {} error: {}", tunnel_id_clone, e);
                        break;
                    }
                    _ => {}
                }
            }

            // Clean up tunnel on disconnect
            Self::cleanup_tunnel(&tunnel_id_clone, &state_clone).await;
        });        // Handle outgoing requests to tunnel client
        let sender_for_outgoing = sender_handle.clone();
        let outgoing_task = tokio::spawn(async move {
            while let Some(request_msg) = _request_receiver.recv().await {
                if let Ok(data) = TunnelProtocol::serialize_message(&request_msg) {
                    let mut sender_guard = sender_for_outgoing.lock().await;
                    if let Err(e) = sender_guard.send(axum::extract::ws::Message::Binary(data)).await {
                        error!("Failed to send request to tunnel: {}", e);
                        break;
                    }
                }
            }
        });

        // Wait for either task to complete
        tokio::select! {
            _ = incoming_task => {},
            _ = outgoing_task => {},
        }
    }    /// Handle individual tunnel protocol messages
    async fn handle_tunnel_message(
        message: TunnelMessage,
        tunnel_id: &str,
        state: &Arc<TunnelServerState>,
        sender: &Arc<tokio::sync::Mutex<futures_util::stream::SplitSink<axum::extract::ws::WebSocket, axum::extract::ws::Message>>>,
    ) {
        match message {
            TunnelMessage::Auth { token, subdomain, protocol_version } => {
                Self::handle_auth_message(tunnel_id, token, subdomain, protocol_version, state, sender).await;
            }
            TunnelMessage::HttpResponse { id, status, headers, body } => {
                Self::handle_http_response(id, status, headers, body, state).await;
            }
            TunnelMessage::Ping { timestamp } => {
                let pong = TunnelMessage::Pong { timestamp };
                Self::send_tunnel_message(&pong, sender).await;
            }
            TunnelMessage::Pong { timestamp: _ } => {
                // Handle pong response - update tunnel health
                debug!("Received pong from tunnel {}", tunnel_id);
            }
            _ => {
                debug!("Unhandled tunnel message type from {}", tunnel_id);
            }
        }
    }    /// Handle HTTP response from tunnel client
    async fn handle_http_response(
        request_id: String,
        status: u16,
        headers: HashMap<String, String>,
        body: Option<Vec<u8>>,
        state: &Arc<TunnelServerState>,
    ) {
        debug!("Received HTTP response for request {}: status {}", request_id, status);

        // Find and remove the pending request
        let pending_request = {
            let mut pending = state.pending_requests.write().await;
            pending.remove(&request_id)
        };

        if let Some(pending_request) = pending_request {
            let response_message = TunnelMessage::HttpResponse {
                id: request_id,
                status,
                headers,
                body,
            };

            // Send response back to the waiting HTTP handler
            if let Err(_) = pending_request.response_sender.send(response_message) {
                warn!("Failed to send response for request {}: receiver dropped", pending_request.id);
            }
        } else {
            warn!("Received response for unknown request ID: {}", request_id);
        }
    }

    /// Convert tunnel response to HTTP response
    fn tunnel_response_to_http(tunnel_response: TunnelMessage) -> Response {
        match tunnel_response {
            TunnelMessage::HttpResponse { status, headers, body, .. } => {
                let mut response_builder = axum::http::Response::builder()
                    .status(status);

                // Add headers
                for (name, value) in headers {
                    if let (Ok(header_name), Ok(header_value)) = (
                        axum::http::HeaderName::from_bytes(name.as_bytes()),
                        axum::http::HeaderValue::from_str(&value)
                    ) {
                        response_builder = response_builder.header(header_name, header_value);
                    }
                }

                // Add body
                let response_body = body.unwrap_or_default();
                response_builder
                    .body(axum::body::Body::from(response_body))
                    .unwrap_or_else(|_| {
                        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to build response").into_response()
                    })
            }
            _ => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Invalid response type").into_response()
            }
        }
    }

    /// Cleanup expired pending requests
    async fn cleanup_expired_requests(state: Arc<TunnelServerState>) {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            
            let now = std::time::Instant::now();
            let mut expired_requests = Vec::new();
            
            {
                let pending = state.pending_requests.read().await;
                for (id, request) in pending.iter() {
                    if now.duration_since(request.created_at) > Duration::from_secs(60) {
                        expired_requests.push(id.clone());
                    }
                }
            }
            
            if !expired_requests.is_empty() {
                let mut pending = state.pending_requests.write().await;
                for id in expired_requests {
                    if let Some(request) = pending.remove(&id) {
                        warn!("Cleaning up expired request: {}", request.id);
                        // The oneshot receiver will get a channel closed error
                    }
                }
            }
        }    }

    /// Handle tunnel authentication
    async fn handle_auth_message(
        tunnel_id: &str,
        token: String,
        requested_subdomain: Option<String>,
        protocol_version: String,
        state: &Arc<TunnelServerState>,
        sender: &Arc<tokio::sync::Mutex<futures_util::stream::SplitSink<axum::extract::ws::WebSocket, axum::extract::ws::Message>>>,
    ) {        // Validate protocol version
        if !state.protocol.is_compatible_version(&protocol_version) {
            let error_msg = TunnelMessage::Error {
                code: 400,
                message: "Incompatible protocol version".to_string(),
            };
            Self::send_tunnel_message(&error_msg, sender).await;
            return;
        }

        // Validate authentication token
        if !Self::validate_auth_token(&token, &state.config) {
            let error_msg = TunnelMessage::Error {
                code: 401,
                message: "Invalid authentication token".to_string(),
            };
            Self::send_tunnel_message(&error_msg, sender).await;
            return;
        }

        // Generate or validate subdomain
        let subdomain = match requested_subdomain {
            Some(requested) => {
                if Self::is_subdomain_available(&requested, state).await {
                    requested
                } else {
                    Self::generate_subdomain(&state.config)
                }
            }
            None => Self::generate_subdomain(&state.config),
        };        // Create tunnel entry
        let (_request_sender, _) = mpsc::channel(100);
        let tunnel = ActiveTunnel {
            id: tunnel_id.to_string(),
            subdomain: subdomain.clone(),
            client_ip: "0.0.0.0".to_string(), // TODO: Extract real IP
            authenticated: true,
            connected_at: std::time::SystemTime::now(),
            request_sender: _request_sender,
        };

        // Register tunnel
        {
            let mut tunnels = state.active_tunnels.write().await;
            tunnels.insert(tunnel_id.to_string(), tunnel);
        }
        {
            let mut mapping = state.subdomain_mapping.write().await;
            mapping.insert(subdomain.clone(), tunnel_id.to_string());
        }

        // Send authentication response
        let auth_response = TunnelMessage::AuthResponse {
            success: true,
            assigned_subdomain: Some(subdomain.clone()),
            error: None,
        };
        Self::send_tunnel_message(&auth_response, sender).await;

        info!("Tunnel {} authenticated with subdomain: {}", tunnel_id, subdomain);
    }    /// Send message through tunnel WebSocket
    async fn send_tunnel_message(
        message: &TunnelMessage,
        sender: &Arc<tokio::sync::Mutex<futures_util::stream::SplitSink<axum::extract::ws::WebSocket, axum::extract::ws::Message>>>,
    ) {
        if let Ok(data) = TunnelProtocol::serialize_message(message) {
            let mut sender_guard = sender.lock().await;
            if let Err(e) = sender_guard.send(axum::extract::ws::Message::Binary(data)).await {
                error!("Failed to send tunnel message: {}", e);
            }
        }
    }

    /// Clean up tunnel on disconnect
    async fn cleanup_tunnel(tunnel_id: &str, state: &Arc<TunnelServerState>) {
        let subdomain = {
            let mut tunnels = state.active_tunnels.write().await;
            match tunnels.remove(tunnel_id) {
                Some(tunnel) => tunnel.subdomain,
                None => return,
            }
        };

        {
            let mut mapping = state.subdomain_mapping.write().await;
            mapping.remove(&subdomain);
        }

        info!("Cleaned up tunnel {} with subdomain {}", tunnel_id, subdomain);
    }

    /// Handle health check endpoint
    async fn handle_health_check(State(state): State<Arc<TunnelServerState>>) -> Response {
        let tunnel_count = state.active_tunnels.read().await.len();
        let response = serde_json::json!({
            "status": "healthy",
            "active_tunnels": tunnel_count,
            "server_config": {
                "base_domain": state.config.base_domain,
                "max_tunnels": state.config.max_tunnels
            }
        });
        (StatusCode::OK, response.to_string()).into_response()
    }

    /// Extract subdomain from host header
    fn extract_subdomain(host: &str, base_domain: &str) -> Option<String> {
        if let Some(subdomain_with_domain) = host.strip_suffix(base_domain) {
            if let Some(subdomain) = subdomain_with_domain.strip_suffix('.') {
                if !subdomain.is_empty() {
                    return Some(subdomain.to_string());
                }
            }
        }
        None
    }

    /// Convert HeaderMap to HashMap
    fn headers_to_hashmap(headers: &HeaderMap) -> HashMap<String, String> {
        headers
            .iter()
            .map(|(name, value)| {
                (
                    name.to_string(),
                    value.to_str().unwrap_or("").to_string(),
                )
            })
            .collect()
    }

    /// Validate authentication token
    fn validate_auth_token(token: &str, config: &TunnelServerConfig) -> bool {
        if !config.auth.required {
            return true;
        }

        config.auth.api_keys.contains(&token.to_string())
    }

    /// Check if subdomain is available
    async fn is_subdomain_available(subdomain: &str, state: &Arc<TunnelServerState>) -> bool {
        let mapping = state.subdomain_mapping.read().await;
        !mapping.contains_key(subdomain)
    }    /// Generate random subdomain
    fn generate_subdomain(config: &TunnelServerConfig) -> String {
        match config.subdomain_strategy {
            SubdomainStrategy::Random => {
                thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(8)
                    .map(char::from)
                    .collect()
            }
            SubdomainStrategy::Uuid => {
                Uuid::new_v4().to_string().replace('-', "")[..8].to_string()
            }
            SubdomainStrategy::UserSpecified => {
                // Fallback to random if user didn't specify
                thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(8)
                    .map(char::from)
                    .collect()
            }
        }    }
}
