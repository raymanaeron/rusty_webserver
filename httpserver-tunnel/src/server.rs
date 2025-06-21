// Phase 7.2 Tunnel Server - Public HTTP Server Integration
// Tunnel server that accepts WebSocket connections from tunnel clients and routes public traffic

use crate::{ TunnelError, config::TunnelServerConfig, protocol::{ TunnelMessage, TunnelProtocol } };
use crate::subdomain::SubdomainManager;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::cmp::min;
use base64::{ Engine as _, engine::general_purpose };
use tokio::sync::{ RwLock, broadcast, mpsc, oneshot };
use futures_util::{ SinkExt, StreamExt };
use uuid::Uuid;
use axum::{
    extract::{ State, WebSocketUpgrade, ConnectInfo },
    http::{ HeaderMap, Method, StatusCode, Uri },
    response::{ IntoResponse, Response },
    routing::{ any, get },
    Router,
};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tracing::{ info, warn, error, debug };
use tokio::net::{ TcpListener, TcpStream };
use tokio::io::{ AsyncReadExt, AsyncWriteExt };

/// TLS stream type for SSL passthrough
pub enum TlsStream {
    Http(TcpStream),
    Https(TcpStream),
}

/// Tunnel connection type for protocol detection
#[derive(Debug, Clone)]
pub enum ConnectionType {
    Http,
    Https,
    WebSocket,
}

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
    pub user_info: Option<String>, // User extracted from token
    pub authenticated: bool,
    pub connected_at: std::time::SystemTime,
    pub request_sender: mpsc::Sender<TunnelMessage>,
}

/// Tunnel server state
#[derive(Debug)]
pub struct TunnelServerState {
    pub config: TunnelServerConfig,
    pub active_tunnels: RwLock<HashMap<String, ActiveTunnel>>,
    pub subdomain_manager: SubdomainManager,
    pub pending_requests: RwLock<HashMap<String, PendingRequest>>, // request_id -> pending_request
    pub active_ssl_connections: RwLock<HashMap<String, mpsc::Sender<Vec<u8>>>>, // connection_id -> ssl_data_sender
    pub rate_limiter: Arc<std::sync::Mutex<RateLimiter>>, // Rate limiting state
    pub protocol: TunnelProtocol,
    pub shutdown_sender: broadcast::Sender<()>,
}

/// Rate limiting state
#[derive(Debug, Default)]
pub struct RateLimiter {
    /// Request counts per tunnel: (count, window_start)
    pub request_counts: HashMap<String, (u32, std::time::Instant)>,
    /// Active connections per tunnel
    pub active_connections: HashMap<String, u32>,
    /// Bandwidth usage per tunnel: (bytes, window_start)
    pub bandwidth_usage: HashMap<String, (u64, std::time::Instant)>,
}

/// Main tunnel server
pub struct TunnelServer {
    config: TunnelServerConfig,
    state: Arc<TunnelServerState>,
}

impl TunnelServer {
    /// Create new tunnel server
    pub fn new(config: TunnelServerConfig) -> ServerResult<Self> {
        let (shutdown_sender, _) = broadcast::channel(100);

        // Create subdomain manager with persistent storage
        let storage_path = std::env
            ::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("tunnel_data")
            .join("subdomains.json");

        let subdomain_manager = SubdomainManager::new(config.clone(), storage_path);
        let state = Arc::new(TunnelServerState {
            config: config.clone(),
            active_tunnels: RwLock::new(HashMap::new()),
            subdomain_manager,
            pending_requests: RwLock::new(HashMap::new()),
            active_ssl_connections: RwLock::new(HashMap::new()),
            rate_limiter: Arc::new(std::sync::Mutex::new(RateLimiter::default())),
            protocol: TunnelProtocol::new(),
            shutdown_sender,
        });

        Ok(Self { config, state })
    }
    /// Start the tunnel server
    pub async fn start(&self) -> ServerResult<()> {
        if !self.config.enabled {
            info!("Tunnel server is disabled");
            return Ok(());
        }

        // Initialize subdomain manager
        self.state.subdomain_manager
            .initialize().await
            .map_err(|e|
                TunnelError::InternalError(format!("Failed to initialize subdomain manager: {}", e))
            )?;

        info!("Starting tunnel server on port {}", self.config.tunnel_port);

        // Create the main router for public traffic
        let public_router = self.create_public_router();

        // Create the tunnel WebSocket endpoint
        let tunnel_router = self.create_tunnel_router();

        // Start SSL passthrough if enabled
        if self.config.ssl.enabled {
            self.start_ssl_passthrough().await?;
        }

        // Start cleanup task for expired requests
        let state_for_cleanup = self.state.clone();
        tokio::spawn(async move {
            Self::cleanup_expired_requests(state_for_cleanup).await;
        }); // Start both servers concurrently
        let public_addr = SocketAddr::new(
            self.config.network.public_bind_address
                .parse()
                .map_err(|e|
                    TunnelError::ConfigError(format!("Invalid public bind address: {}", e))
                )?,
            self.config.public_port
        );
        let tunnel_addr = SocketAddr::new(
            self.config.network.bind_address
                .parse()
                .map_err(|e|
                    TunnelError::ConfigError(format!("Invalid tunnel bind address: {}", e))
                )?,
            self.config.tunnel_port
        );

        info!("Tunnel server listening for public traffic on {}", public_addr);
        info!("Tunnel server listening for tunnel connections on {}", tunnel_addr);

        // Run both servers
        let public_server = axum::serve(
            tokio::net::TcpListener
                ::bind(public_addr).await
                .map_err(|e|
                    TunnelError::NetworkError(format!("Failed to bind public address: {}", e))
                )?,
            public_router
        );

        let tunnel_server = axum::serve(
            tokio::net::TcpListener
                ::bind(tunnel_addr).await
                .map_err(|e|
                    TunnelError::NetworkError(format!("Failed to bind tunnel address: {}", e))
                )?,
            tunnel_router.into_make_service_with_connect_info::<SocketAddr>()
        );

        // Run both servers concurrently
        tokio::try_join!(
            async {
                public_server.await.map_err(|e| TunnelError::NetworkError(e.to_string()))
            },
            async {
                tunnel_server.await.map_err(|e| TunnelError::NetworkError(e.to_string()))
            }
        )?;

        Ok(())
    }    /// Create router for public traffic (subdomain routing)
    fn create_public_router(&self) -> Router {
        Router::new()
            .route("/", any(Self::handle_public_request))
            .route("/*path", any(Self::handle_public_request))
            .layer(
                ServiceBuilder::new()
                    .layer(CorsLayer::permissive())
                    // Add middleware to ensure HTTP-only responses
                    .layer(axum::middleware::from_fn(Self::add_http_headers))
                    .into_inner()
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
        body: axum::body::Bytes
    ) -> Response {
        // Extract subdomain or custom domain from Host header
        let host = match headers.get("host") {
            Some(host) => host.to_str().unwrap_or(""),
            None => {
                error!("HTTP request missing Host header");
                return (StatusCode::BAD_REQUEST, "Missing Host header").into_response();
            }
        };

        info!("Public HTTP request: {} {} from host: {}", method, uri, host);

        // Try to find tunnel by subdomain first, then by custom domain
        let subdomain = Self::extract_subdomain(host, &state.config.base_domain);
        info!("Extracted subdomain: {:?} from host: {} with base domain: {}", subdomain, host, state.config.base_domain);
        
        let tunnel_id = if let Some(subdomain) = &subdomain {
            // Standard subdomain routing (e.g., abc123.httpserver.io)
            let tunnel_for_subdomain = state.subdomain_manager.get_tunnel_for_subdomain(subdomain).await;
            info!("Tunnel for subdomain '{}': {:?}", subdomain, tunnel_for_subdomain);
            tunnel_for_subdomain
        } else {
            // Check if it's a custom domain (e.g., myapp.com)
            info!("No subdomain extracted, checking custom domain for: {}", host);
            state.subdomain_manager.get_tunnel_for_custom_domain(host).await
        };        let tunnel_id = match tunnel_id {
            Some(id) => {
                info!("Found tunnel ID: {} for host: {}", id, host);
                id
            },
            None => {
                // Debug: Show all active tunnels
                let tunnels = state.active_tunnels.read().await;
                let tunnel_count = tunnels.len();
                info!("No tunnel found for host: {}. Active tunnels ({}): {:?}", 
                      host, tunnel_count, 
                      tunnels.keys().collect::<Vec<_>>());
                if tunnel_count > 0 {
                    for (tid, tunnel) in tunnels.iter() {
                        info!("  Tunnel {}: subdomain '{}', authenticated: {}", 
                              tid, tunnel.subdomain, tunnel.authenticated);
                    }
                }
                return (StatusCode::NOT_FOUND, "Tunnel not found for this domain").into_response();
            }
        };// Get the tunnel connection
        let tunnel = {
            let tunnels = state.active_tunnels.read().await;
            match tunnels.get(&tunnel_id) {
                Some(tunnel) => tunnel.clone(),
                None => {
                    return (StatusCode::BAD_GATEWAY, "Tunnel disconnected").into_response();
                }
            }
        };

        // Apply rate limiting if enabled
        if state.config.rate_limiting.enabled {
            if let Err(rate_limit_error) = Self::check_rate_limit(&tunnel_id, &state).await {
                return (StatusCode::TOO_MANY_REQUESTS, rate_limit_error).into_response();
            }
        }

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
            path: uri
                .path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or(uri.path())
                .to_string(),
            headers: Self::headers_to_hashmap(&headers),
            body: if body.is_empty() {
                None
            } else {
                Some(body.to_vec())
            },
            client_ip: "0.0.0.0".to_string(), // TODO: Extract real client IP
        };

        // Send request to tunnel client
        if let Err(e) = tunnel.request_sender.send(request_message).await {
            error!("Failed to send request to tunnel: {}", e);
            // Clean up pending request
            state.pending_requests.write().await.remove(&request_id);
            return (StatusCode::BAD_GATEWAY, "Tunnel communication error").into_response();
        } // Wait for response with timeout
        let response_timeout = Duration::from_secs(30);
        let tunnel_id_for_rate_limit = tunnel_id.clone();
        let state_for_rate_limit = state.clone();

        match tokio::time::timeout(response_timeout, response_receiver).await {
            Ok(Ok(tunnel_response)) => {
                // Calculate bytes transferred for rate limiting
                let bytes_transferred = match &tunnel_response {
                    TunnelMessage::HttpResponse { body, .. } => {
                        body.as_ref()
                            .map(|b| b.len() as u64)
                            .unwrap_or(0)
                    }
                    _ => 0,
                };

                // Update rate limiting counters
                Self::update_rate_limit_completion(
                    &tunnel_id_for_rate_limit,
                    &state_for_rate_limit,
                    bytes_transferred
                ).await;

                // Convert tunnel response to HTTP response
                Self::tunnel_response_to_http(tunnel_response)
            }
            Ok(Err(_)) => {
                // Channel closed - still update rate limiting
                Self::update_rate_limit_completion(
                    &tunnel_id_for_rate_limit,
                    &state_for_rate_limit,
                    0
                ).await;
                (StatusCode::BAD_GATEWAY, "Tunnel closed during request").into_response()
            }
            Err(_) => {
                // Timeout - still update rate limiting
                Self::update_rate_limit_completion(
                    &tunnel_id_for_rate_limit,
                    &state_for_rate_limit,
                    0
                ).await;
                state.pending_requests.write().await.remove(&request_id);
                (StatusCode::GATEWAY_TIMEOUT, "Request timeout").into_response()
            }
        }
    }
    /// Handle tunnel WebSocket connection establishment
    async fn handle_tunnel_connection(
        ws: WebSocketUpgrade,
        State(state): State<Arc<TunnelServerState>>,
        ConnectInfo(addr): ConnectInfo<SocketAddr>
    ) -> Response {
        info!("New tunnel connection attempt from {}", addr);
        ws.on_upgrade(move |socket| Self::handle_tunnel_websocket(socket, state, addr.to_string()))
    }
    /// Handle tunnel WebSocket communication
    async fn handle_tunnel_websocket(
        socket: axum::extract::ws::WebSocket,
        state: Arc<TunnelServerState>,
        client_ip: String
    ) {
        let (sender, mut receiver) = socket.split();
        let tunnel_id = Uuid::new_v4().to_string();

        info!("New tunnel connection: {} from {}", tunnel_id, client_ip);

        // Create channel for sending requests to this tunnel
        let (request_sender, mut request_receiver) = mpsc::channel::<TunnelMessage>(100);

        // Store request sender for authentication phase
        let request_sender_for_auth = request_sender.clone();

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
                        if
                            let Ok(tunnel_msg) = TunnelProtocol::deserialize_message(
                                text.as_bytes()
                            )
                        {
                            Self::handle_tunnel_message(
                                tunnel_msg,
                                &tunnel_id_clone,
                                &state_clone,
                                &sender_for_incoming,
                                &request_sender_for_auth
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
                                &request_sender_for_auth
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
        }); // Handle outgoing requests to tunnel client
        let sender_for_outgoing = sender_handle.clone();
        let outgoing_task = tokio::spawn(async move {
            while let Some(request_msg) = request_receiver.recv().await {
                match serde_json::to_string(&request_msg) {
                    Ok(text) => {
                        let mut sender_guard = sender_for_outgoing.lock().await;
                        if
                            let Err(e) = sender_guard.send(
                                axum::extract::ws::Message::Text(text)
                            ).await
                        {
                            error!("Failed to send request to tunnel: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to serialize request message: {}", e);
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
    }
    /// Handle individual tunnel protocol messages
    async fn handle_tunnel_message(
        message: TunnelMessage,
        tunnel_id: &str,
        state: &Arc<TunnelServerState>,
        sender: &Arc<
            tokio::sync::Mutex<
                futures_util::stream::SplitSink<
                    axum::extract::ws::WebSocket,
                    axum::extract::ws::Message
                >
            >
        >,
        request_sender: &mpsc::Sender<TunnelMessage>
    ) {
        match message {
            TunnelMessage::Auth { token, subdomain, protocol_version } => {
                Self::handle_auth_message(
                    tunnel_id,
                    token,
                    subdomain,
                    protocol_version,
                    state,
                    sender,
                    request_sender.clone()
                ).await;
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
            TunnelMessage::SslData { id, data } => {
                // Handle SSL data forwarding
                debug!("Received SSL data for connection {}: {} bytes", id, data.len());

                // Forward SSL data to the appropriate connection
                let ssl_connections = state.active_ssl_connections.read().await;
                if let Some(sender) = ssl_connections.get(&id) {
                    if let Err(e) = sender.send(data).await {
                        warn!("Failed to forward SSL data to connection {}: {}", id, e);
                    }
                } else {
                    warn!("No active SSL connection found for ID: {}", id);
                }
            }
            TunnelMessage::SslClose { id } => {
                // Handle SSL connection close
                debug!("SSL connection {} closed", id);

                // Close the appropriate SSL connection by dropping the sender
                let mut ssl_connections = state.active_ssl_connections.write().await;
                if ssl_connections.remove(&id).is_some() {
                    debug!("Cleaned up SSL connection {}", id);
                } else {
                    warn!("No SSL connection found to close for ID: {}", id);
                }
            }
            _ => {
                debug!("Unhandled tunnel message type from {}", tunnel_id);
            }
        }
    }
    /// Handle HTTP response from tunnel client
    async fn handle_http_response(
        request_id: String,
        status: u16,
        headers: HashMap<String, String>,
        body: Option<Vec<u8>>,
        state: &Arc<TunnelServerState>
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
                warn!(
                    "Failed to send response for request {}: receiver dropped",
                    pending_request.id
                );
            }
        } else {
            warn!("Received response for unknown request ID: {}", request_id);
        }
    }

    /// Convert tunnel response to HTTP response
    fn tunnel_response_to_http(tunnel_response: TunnelMessage) -> Response {
        match tunnel_response {
            TunnelMessage::HttpResponse { status, headers, body, .. } => {
                let mut response_builder = axum::http::Response::builder().status(status); // Add headers
                for (name, value) in headers {
                    if
                        let (Ok(header_name), Ok(header_value)) = (
                            axum::http::HeaderName::from_bytes(name.as_bytes()),
                            axum::http::HeaderValue::from_str(&value),
                        )
                    {
                        response_builder = response_builder.header(header_name, header_value);
                    }
                }

                // Add explicit HTTP-only headers to prevent HTTPS redirects
                response_builder = response_builder
                    .header("X-Frame-Options", "SAMEORIGIN")
                    .header("X-Content-Type-Options", "nosniff")
                    .header("Referrer-Policy", "strict-origin-when-cross-origin");

                // Add body
                let response_body = body.unwrap_or_default();
                response_builder
                    .body(axum::body::Body::from(response_body))
                    .unwrap_or_else(|_| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to build response",
                        ).into_response()
                    })
            }
            _ => { (StatusCode::INTERNAL_SERVER_ERROR, "Invalid response type").into_response() }
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
        }
    }
    /// Handle tunnel authentication
    async fn handle_auth_message(
        tunnel_id: &str,
        token: String,
        requested_subdomain: Option<String>,
        protocol_version: String,
        state: &Arc<TunnelServerState>,
        sender: &Arc<
            tokio::sync::Mutex<
                futures_util::stream::SplitSink<
                    axum::extract::ws::WebSocket,
                    axum::extract::ws::Message
                >
            >
        >,
        request_sender: mpsc::Sender<TunnelMessage>
    ) {
        // Validate protocol version
        if !state.protocol.is_compatible_version(&protocol_version) {
            let error_msg = TunnelMessage::Error {
                code: 400,
                message: "Incompatible protocol version".to_string(),
            };
            Self::send_tunnel_message(&error_msg, sender).await;
            return;
        } // Validate authentication token
        if !Self::validate_auth_token(&token, &state.config) {
            let error_msg = TunnelMessage::Error {
                code: 401,
                message: "Invalid authentication token".to_string(),
            };
            Self::send_tunnel_message(&error_msg, sender).await;
            return;
        } // Extract user information from token for logging purposes only
        let user_info = Self::extract_user_info(&token, &state.config);

        // Use the requested subdomain as-is, don't derive from user info
        let preferred_subdomain = requested_subdomain;

        info!("Authenticating tunnel {} for user: {:?}", tunnel_id, user_info); // Allocate subdomain using SubdomainManager
        let subdomain = match
            state.subdomain_manager.allocate_subdomain(
                tunnel_id,
                preferred_subdomain,
                Some("0.0.0.0".to_string()) // TODO: Extract real client IP
            ).await
        {
            Ok(subdomain) => subdomain,
            Err(e) => {
                let error_msg = TunnelMessage::AuthResponse {
                    success: false,
                    assigned_subdomain: None,
                    error: Some(format!("Subdomain allocation failed: {}", e)),
                };
                Self::send_tunnel_message(&error_msg, sender).await;
                return;
            }
        }; // Create tunnel entry
        let tunnel = ActiveTunnel {
            id: tunnel_id.to_string(),
            subdomain: subdomain.clone(),
            client_ip: "0.0.0.0".to_string(), // TODO: Extract real IP
            user_info: user_info.clone(),
            authenticated: true,
            connected_at: std::time::SystemTime::now(),
            request_sender,
        }; // Register tunnel
        {
            let mut tunnels = state.active_tunnels.write().await;
            tunnels.insert(tunnel_id.to_string(), tunnel);
        }

        // Send authentication response
        let auth_response = TunnelMessage::AuthResponse {
            success: true,
            assigned_subdomain: Some(subdomain.clone()),
            error: None,
        };
        Self::send_tunnel_message(&auth_response, sender).await;

        info!(
            "Tunnel {} authenticated with subdomain: {} (user: {:?})",
            tunnel_id,
            subdomain,
            user_info
        );
    }
    /// Send message through tunnel WebSocket
    async fn send_tunnel_message(
        message: &TunnelMessage,
        sender: &Arc<
            tokio::sync::Mutex<
                futures_util::stream::SplitSink<
                    axum::extract::ws::WebSocket,
                    axum::extract::ws::Message
                >
            >
        >
    ) {
        match serde_json::to_string(message) {
            Ok(text) => {
                let mut sender_guard = sender.lock().await;
                if let Err(e) = sender_guard.send(axum::extract::ws::Message::Text(text)).await {
                    error!("Failed to send tunnel message: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to serialize tunnel message: {}", e);
            }
        }
    }

    /// Clean up tunnel on disconnect
    async fn cleanup_tunnel(tunnel_id: &str, state: &Arc<TunnelServerState>) {
        let subdomain = {
            let mut tunnels = state.active_tunnels.write().await;
            match tunnels.remove(tunnel_id) {
                Some(tunnel) => tunnel.subdomain,
                None => {
                    return;
                }
            }
        };

        // Release subdomain using SubdomainManager
        if let Err(e) = state.subdomain_manager.release_subdomain(&subdomain).await {
            warn!("Failed to release subdomain {}: {}", subdomain, e);
        }

        info!("Cleaned up tunnel {} with subdomain {}", tunnel_id, subdomain);
    }

    /// Handle health check endpoint
    async fn handle_health_check(State(state): State<Arc<TunnelServerState>>) -> Response {
        let tunnel_count = state.active_tunnels.read().await.len();
        let response =
            serde_json::json!({
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
            .map(|(name, value)| { (name.to_string(), value.to_str().unwrap_or("").to_string()) })
            .collect()
    }
    /// Validate authentication token (API key or JWT)
    fn validate_auth_token(token: &str, config: &TunnelServerConfig) -> bool {
        if !config.auth.required {
            return true;
        }

        // First check if it's a valid API key
        if config.auth.api_keys.contains(&token.to_string()) {
            return true;
        }

        // If JWT is enabled, try to validate as JWT token
        if config.auth.jwt_enabled {
            if let Some(jwt_secret) = &config.auth.jwt_secret {
                if Self::validate_jwt_token(token, jwt_secret) {
                    return true;
                }
            }
        }

        false
    }
    /// Simple JWT token validation
    fn validate_jwt_token(token: &str, _secret: &str) -> bool {
        // For simplicity, just check if token starts with expected format and decode basic claims
        if !token.starts_with("eyJ") {
            return false;
        }

        // Split JWT token (header.payload.signature)
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return false;
        } // Try to decode payload (basic validation)
        if let Ok(payload_bytes) = general_purpose::URL_SAFE_NO_PAD.decode(parts[1]) {
            if let Ok(payload_str) = std::str::from_utf8(&payload_bytes) {
                if let Ok(claims) = serde_json::from_str::<serde_json::Value>(payload_str) {
                    // Check basic JWT structure and expiration if present
                    if claims.is_object() {
                        if let Some(exp) = claims.get("exp").and_then(|e| e.as_i64()) {
                            let now = std::time::SystemTime
                                ::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64;

                            return now < exp;
                        }
                        return true; // No expiration, consider valid
                    }
                }
            }
        }

        false
    }

    /// Extract user info from token (API key or JWT)
    fn extract_user_info(token: &str, config: &TunnelServerConfig) -> Option<String> {
        // For API keys, we can create a simple mapping
        if config.auth.api_keys.contains(&token.to_string()) {
            // Simple user extraction - use first part of API key or static mapping
            if token.starts_with("sk-") {
                return Some(format!("user-{}", &token[3..min(token.len(), 13)]));
            } else {
                return Some(format!("user-{}", &token[..min(token.len(), 10)]));
            }
        }

        // For JWT tokens, extract from claims
        if config.auth.jwt_enabled && token.starts_with("eyJ") {
            let parts: Vec<&str> = token.split('.').collect();
            if parts.len() == 3 {
                if let Ok(payload_bytes) = general_purpose::URL_SAFE_NO_PAD.decode(parts[1]) {
                    if let Ok(payload_str) = std::str::from_utf8(&payload_bytes) {
                        if let Ok(claims) = serde_json::from_str::<serde_json::Value>(payload_str) {
                            if let Some(sub) = claims.get("sub").and_then(|s| s.as_str()) {
                                return Some(sub.to_string());
                            }
                            if let Some(username) = claims.get("username").and_then(|u| u.as_str()) {
                                return Some(username.to_string());
                            }
                        }
                    }
                }
            }
        }

        None
    }
    /// Check rate limiting for a tunnel
    async fn check_rate_limit(
        tunnel_id: &str,
        state: &Arc<TunnelServerState>
    ) -> Result<(), String> {
        let config = &state.config.rate_limiting;
        let now = std::time::Instant::now();

        let mut rate_limiter = state.rate_limiter.lock().unwrap();

        // Check request rate limit
        let should_reset_window;
        let current_count;
        let current_connections;

        {
            let (count, window_start) = rate_limiter.request_counts
                .entry(tunnel_id.to_string())
                .or_insert((0, now));

            // Check if window should be reset
            should_reset_window = now.duration_since(*window_start) > Duration::from_secs(60);
            current_count = *count;
        }

        // Reset window if needed
        if should_reset_window {
            if let Some((count, window_start)) = rate_limiter.request_counts.get_mut(tunnel_id) {
                *count = 0;
                *window_start = now;
            }
        }

        // Check rate limit
        if current_count >= config.requests_per_minute && !should_reset_window {
            return Err("Request rate limit exceeded".to_string());
        }

        // Check concurrent connections
        current_connections = rate_limiter.active_connections.get(tunnel_id).copied().unwrap_or(0);

        if current_connections >= config.max_concurrent_connections {
            return Err("Concurrent connection limit exceeded".to_string());
        }

        // Increment counters
        if let Some((count, _)) = rate_limiter.request_counts.get_mut(tunnel_id) {
            *count += 1;
        }

        rate_limiter.active_connections
            .entry(tunnel_id.to_string())
            .and_modify(|c| {
                *c += 1;
            })
            .or_insert(1);

        Ok(())
    }

    /// Update rate limiting counters after request completion
    async fn update_rate_limit_completion(
        tunnel_id: &str,
        state: &Arc<TunnelServerState>,
        bytes_transferred: u64
    ) {
        let mut rate_limiter = state.rate_limiter.lock().unwrap();

        // Decrement active connections
        if let Some(count) = rate_limiter.active_connections.get_mut(tunnel_id) {
            if *count > 0 {
                *count -= 1;
            }
        }

        // Update bandwidth usage
        let now = std::time::Instant::now();
        let (bytes, window_start) = rate_limiter.bandwidth_usage
            .entry(tunnel_id.to_string())
            .or_insert((0, now));

        // Reset window if it's been more than a second
        if now.duration_since(*window_start) > Duration::from_secs(1) {
            *bytes = 0;
            *window_start = now;
        }

        *bytes += bytes_transferred;
    }

    /// Start SSL passthrough listener for HTTPS traffic
    pub async fn start_ssl_passthrough(&self) -> ServerResult<()> {
        if !self.config.ssl.enabled {
            info!("SSL passthrough is disabled");
            return Ok(());
        }

        let ssl_addr = SocketAddr::new(
            self.config.network.public_bind_address
                .parse()
                .map_err(|e| TunnelError::ConfigError(format!("Invalid SSL bind address: {}", e)))?,
            443 // Standard HTTPS port
        );

        info!("Starting SSL passthrough on {}", ssl_addr);

        let listener = TcpListener::bind(ssl_addr).await.map_err(|e|
            TunnelError::NetworkError(format!("Failed to bind SSL address: {}", e))
        )?;

        let state = self.state.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        let state_clone = state.clone();
                        tokio::spawn(async move {
                            if
                                let Err(e) = Self::handle_ssl_connection(
                                    stream,
                                    addr,
                                    state_clone
                                ).await
                            {
                                error!("SSL connection error from {}: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept SSL connection: {}", e);
                    }
                }
            }
        });

        Ok(())
    }
    /// Handle SSL/TLS connections for passthrough
    async fn handle_ssl_connection(
        mut stream: TcpStream,
        _client_addr: SocketAddr,
        state: Arc<TunnelServerState>
    ) -> ServerResult<()> {
        // Read the initial bytes to detect TLS handshake and extract SNI
        let mut buffer = [0u8; 1024];
        let bytes_read = stream
            .read(&mut buffer).await
            .map_err(|e|
                TunnelError::NetworkError(format!("Failed to read TLS handshake: {}", e))
            )?;

        if bytes_read == 0 {
            return Err(TunnelError::NetworkError("Empty TLS handshake".to_string()));
        }

        // Extract SNI (Server Name Indication) from TLS handshake
        let hostname = Self::extract_sni_from_tls(&buffer[..bytes_read]).ok_or_else(||
            TunnelError::ValidationError("Could not extract SNI from TLS handshake".to_string())
        )?;

        // Extract subdomain from hostname
        let subdomain = Self::extract_subdomain(&hostname, &state.config.base_domain).ok_or_else(||
            TunnelError::ValidationError("Invalid subdomain in SNI".to_string())
        )?;

        // Find the tunnel for this subdomain
        let tunnel_id = state.subdomain_manager
            .get_tunnel_for_subdomain(&subdomain).await
            .ok_or_else(||
                TunnelError::ValidationError("No tunnel found for subdomain".to_string())
            )?;

        // Get the tunnel connection
        let tunnel = {
            let tunnels = state.active_tunnels.read().await;
            tunnels
                .get(&tunnel_id)
                .cloned()
                .ok_or_else(|| TunnelError::ValidationError("Tunnel disconnected".to_string()))?
        };

        // Forward the TLS traffic through the tunnel
        Self::forward_ssl_through_tunnel(stream, buffer[..bytes_read].to_vec(), tunnel, state).await
    }

    /// Extract SNI (Server Name Indication) from TLS Client Hello
    fn extract_sni_from_tls(data: &[u8]) -> Option<String> {
        // TLS handshake parsing to extract SNI
        // This is a simplified version - in production, you'd want a more robust parser

        if data.len() < 43 {
            return None;
        }

        // Check if it's a TLS handshake record (type 22)
        if data[0] != 0x16 {
            return None;
        }

        // Check if it's a Client Hello (type 1)
        if data[5] != 0x01 {
            return None;
        }

        // Parse through the handshake to find extensions
        let mut offset = 43; // Skip to session ID length

        // Skip session ID
        if offset >= data.len() {
            return None;
        }
        offset += (data[offset] as usize) + 1;

        // Skip cipher suites
        if offset + 1 >= data.len() {
            return None;
        }
        let cipher_suites_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2 + cipher_suites_len;

        // Skip compression methods
        if offset >= data.len() {
            return None;
        }
        offset += (data[offset] as usize) + 1;

        // Check for extensions
        if offset + 1 >= data.len() {
            return None;
        }
        let extensions_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;

        let extensions_end = offset + extensions_len;
        while offset + 3 < extensions_end && offset + 3 < data.len() {
            let ext_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let ext_len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
            offset += 4;

            // SNI extension type is 0
            if ext_type == 0 && offset + ext_len <= data.len() {
                return Self::parse_sni_extension(&data[offset..offset + ext_len]);
            }

            offset += ext_len;
        }

        None
    }

    /// Parse SNI extension data
    fn parse_sni_extension(data: &[u8]) -> Option<String> {
        if data.len() < 5 {
            return None;
        }

        // Skip server name list length (2 bytes)
        let mut offset = 2;

        // Check name type (1 byte) - should be 0 for hostname
        if data[offset] != 0 {
            return None;
        }
        offset += 1;

        // Get hostname length (2 bytes)
        let hostname_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;

        if offset + hostname_len <= data.len() {
            String::from_utf8(data[offset..offset + hostname_len].to_vec()).ok()
        } else {
            None
        }
    }
    /// Forward SSL traffic through tunnel WebSocket
    async fn forward_ssl_through_tunnel(
        client_stream: TcpStream,
        initial_data: Vec<u8>,
        tunnel: ActiveTunnel,
        state: Arc<TunnelServerState>
    ) -> ServerResult<()> {
        // Generate unique connection ID for this SSL session
        let connection_id = Uuid::new_v4().to_string();

        // Create TLS tunnel message
        let ssl_connect_message = TunnelMessage::SslConnect {
            id: connection_id.clone(),
            initial_data: Some(initial_data),
        };

        // Send SSL connection request to tunnel client
        if let Err(e) = tunnel.request_sender.send(ssl_connect_message).await {
            return Err(TunnelError::NetworkError(format!("Failed to send SSL connect: {}", e)));
        }

        info!("SSL connection {} established for tunnel {}", connection_id, tunnel.id);

        // Create channels for bidirectional SSL data forwarding
        let (ssl_tx, mut ssl_rx) = mpsc::channel::<Vec<u8>>(100);
        let connection_id_for_state = connection_id.clone();

        // Store SSL connection for receiving data from tunnel client
        {
            let mut ssl_connections = state.active_ssl_connections.write().await;
            ssl_connections.insert(connection_id.clone(), ssl_tx);
        }

        // Split TCP stream for bidirectional communication
        let (mut read_half, mut write_half) = client_stream.into_split();

        // Task 1: Forward data from client to tunnel
        let tunnel_sender = tunnel.request_sender.clone();
        let connection_id_read = connection_id.clone();
        let client_to_tunnel_task = tokio::spawn(async move {
            let mut buffer = [0u8; 8192];
            loop {
                match read_half.read(&mut buffer).await {
                    Ok(0) => {
                        // Client closed connection
                        debug!("SSL client connection {} closed", connection_id_read);
                        break;
                    }
                    Ok(n) => {
                        let data = buffer[..n].to_vec();
                        let ssl_data_msg = TunnelMessage::SslData {
                            id: connection_id_read.clone(),
                            data,
                        };

                        if let Err(e) = tunnel_sender.send(ssl_data_msg).await {
                            error!("Failed to forward SSL data to tunnel: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to read from SSL client: {}", e);
                        break;
                    }
                }
            }

            // Send SSL close message
            let ssl_close_msg = TunnelMessage::SslClose {
                id: connection_id_read,
            };
            let _ = tunnel_sender.send(ssl_close_msg).await;
        });

        // Task 2: Forward data from tunnel to client
        let connection_id_write = connection_id.clone();
        let tunnel_to_client_task = tokio::spawn(async move {
            while let Some(data) = ssl_rx.recv().await {
                if let Err(e) = write_half.write_all(&data).await {
                    error!("Failed to write SSL data to client: {}", e);
                    break;
                }
            }
            debug!("SSL tunnel to client forwarding stopped for {}", connection_id_write);
        });

        // Wait for either task to complete (connection closed)
        tokio::select! {
            _ = client_to_tunnel_task => {},
            _ = tunnel_to_client_task => {},
        }

        // Cleanup SSL connection state
        {
            let mut ssl_connections = state.active_ssl_connections.write().await;
            ssl_connections.remove(&connection_id_for_state);
        }

        info!("SSL connection {} forwarding completed", connection_id);
        Ok(())
    }

    /// Middleware to add HTTP-only headers and prevent HTTPS redirects
    async fn add_http_headers(
        request: axum::extract::Request,
        next: axum::middleware::Next
    ) -> Response {
        let mut response = next.run(request).await;

        // Add headers to prevent HTTPS redirects and ensure HTTP-only operation
        let headers = response.headers_mut();
        headers.insert("X-Frame-Options", "SAMEORIGIN".parse().unwrap());
        headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
        headers.insert("Referrer-Policy", "strict-origin-when-cross-origin".parse().unwrap());
        headers.insert("X-Forwarded-Proto", "http".parse().unwrap());

        // Explicitly remove any HSTS headers that might be added elsewhere
        headers.remove("Strict-Transport-Security");

        response
    }
}
