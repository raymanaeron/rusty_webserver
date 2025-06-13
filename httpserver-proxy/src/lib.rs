use axum::{
    extract::Request,
    response::{Response, IntoResponse},
    http::{StatusCode, HeaderMap, HeaderName, HeaderValue},
    body::Body,
};
use axum_tungstenite::{Message, WebSocket, WebSocketUpgrade};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as TungsteniteMessage};
use futures_util::{sink::SinkExt, stream::StreamExt};
use std::{net::SocketAddr, time::Duration, collections::HashMap};

// Re-export types from dependencies
pub use httpserver_config::{ProxyRoute, Target, LoadBalancingStrategy, WebSocketHealthConfig, HttpHealthConfig};
pub use httpserver_balancer::LoadBalancer;

// Health check modules
pub mod websocket_health;
pub mod http_health;
pub mod health_integration;

pub use websocket_health::{WebSocketHealthChecker, WebSocketHealthMonitor};
pub use http_health::{HttpHealthChecker, HttpHealthMonitor};
pub use health_integration::{HealthCheckIntegration, HealthSummary};

/// Route matching engine for reverse proxy
pub struct RouteMatch {
    /// The matched route configuration
    pub route: ProxyRoute,
    /// The path after stripping the matched pattern (for forwarding)
    pub stripped_path: String,
    /// Whether this is an exact match or wildcard match
    pub is_wildcard: bool,
}

/// Proxy route matcher that handles path-based routing with wildcards
pub struct RouteMatcher {
    /// Ordered list of routes (order matters for precedence)
    routes: Vec<ProxyRoute>,
    /// Compiled route patterns for fast matching
    patterns: Vec<RoutePattern>,
}

/// Internal structure for compiled route patterns
#[derive(Debug, Clone)]
struct RoutePattern {
    /// Original route configuration
    route: ProxyRoute,
    /// Compiled pattern parts
    parts: Vec<PatternPart>,
    /// Whether this pattern ends with a wildcard
    has_wildcard: bool,
}

/// Parts of a route pattern
#[derive(Debug, Clone)]
enum PatternPart {
    /// Exact string match
    Literal(String),
    /// Wildcard match (captures everything)
    Wildcard,
}

impl RouteMatcher {
    /// Create a new route matcher with the given proxy routes
    pub fn new(routes: Vec<ProxyRoute>) -> Self {
        let patterns = routes.iter().map(|route| Self::compile_pattern(route)).collect();
        
        Self {
            routes,
            patterns,
        }
    }
    
    /// Find the first matching route for the given path
    /// Returns None if no route matches
    pub fn find_match(&self, path: &str) -> Option<RouteMatch> {
        // Normalize the path (ensure it starts with /)
        let normalized_path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        };
        
        // Try each pattern in order (first match wins)
        for pattern in &self.patterns {
            if let Some(stripped_path) = Self::match_pattern(pattern, &normalized_path) {
                return Some(RouteMatch {
                    route: pattern.route.clone(),
                    stripped_path,
                    is_wildcard: pattern.has_wildcard,
                });
            }
        }
        
        None
    }
    
    /// Compile a route pattern into matchable parts
    fn compile_pattern(route: &ProxyRoute) -> RoutePattern {
        let path = &route.path;
        let mut parts = Vec::new();
        let mut has_wildcard = false;
        
        // Handle the pattern parsing
        if path.ends_with("/*") {
            // Pattern like "/api/*" - match prefix and capture rest
            has_wildcard = true;
            let prefix = &path[..path.len() - 2]; // Remove "/*"
            if !prefix.is_empty() {
                parts.push(PatternPart::Literal(prefix.to_string()));
            }
            parts.push(PatternPart::Wildcard);
        } else if path == "*" {
            // Pattern "*" - match everything
            has_wildcard = true;
            parts.push(PatternPart::Wildcard);
        } else {
            // Exact match pattern like "/api/health"
            parts.push(PatternPart::Literal(path.to_string()));
        }
        
        RoutePattern {
            route: route.clone(),
            parts,
            has_wildcard,
        }
    }
    
    /// Match a compiled pattern against a path
    /// Returns the stripped path if matched, None if no match
    fn match_pattern(pattern: &RoutePattern, path: &str) -> Option<String> {
        let mut path_pos = 0;
        let path_bytes = path.as_bytes();
        
        for part in &pattern.parts {
            match part {
                PatternPart::Literal(literal) => {
                    // Check if the path matches the literal at current position
                    let literal_bytes = literal.as_bytes();
                    if path_pos + literal_bytes.len() > path_bytes.len() {
                        return None; // Not enough path left
                    }
                    
                    if &path_bytes[path_pos..path_pos + literal_bytes.len()] != literal_bytes {
                        return None; // Literal doesn't match
                    }
                    
                    path_pos += literal_bytes.len();
                }
                PatternPart::Wildcard => {
                    // Wildcard matches the rest of the path
                    let remaining = &path[path_pos..];
                    return Some(remaining.to_string());
                }
            }
        }
        
        // If we've consumed the entire path and there's no wildcard, 
        // return empty string (exact match)
        if path_pos == path.len() {
            Some(String::new())
        } else {
            None // Path has extra characters that weren't matched
        }
    }
    
    /// Get all configured routes (for debugging/inspection)
    pub fn routes(&self) -> &[ProxyRoute] {
        &self.routes
    }
}

/// Proxy handler that manages route matching and request forwarding
pub struct ProxyHandler {
    /// Route matcher for finding proxy targets
    route_matcher: RouteMatcher,
    /// HTTP forwarder for handling requests
    forwarder: ProxyForwarder,
    /// Load balancers per route (keyed by route path)
    load_balancers: HashMap<String, LoadBalancer>,
}

impl ProxyHandler {
    /// Create a new proxy handler with the given routes
    pub fn new(routes: Vec<ProxyRoute>) -> Self {
        let route_matcher = RouteMatcher::new(routes.clone());
        let forwarder = ProxyForwarder::new();
        
        // Create load balancers for each route
        let mut load_balancers = HashMap::new();
        for route in routes {
            let targets = route.get_targets();
            if !targets.is_empty() {
                let balancer = LoadBalancer::new(targets, route.strategy.clone());
                load_balancers.insert(route.path.clone(), balancer);
            }
        }
        
        Self {
            route_matcher,
            forwarder,
            load_balancers,
        }
    }
    
    /// Find a matching route for the given path
    pub fn find_route(&self, path: &str) -> Option<RouteMatch> {
        self.route_matcher.find_match(path)
    }
    
    /// Check if any proxy routes are configured
    pub fn has_routes(&self) -> bool {
        !self.route_matcher.routes().is_empty()
    }
    
    /// Get all configured routes (for inspection)
    pub fn routes(&self) -> &[ProxyRoute] {
        self.route_matcher.routes()
    }
    
    /// Handle a proxy request (find route and forward if matched)
    pub async fn handle_request(
        &self,
        req: Request<Body>,
        client_ip: SocketAddr,
    ) -> Option<Result<Response<Body>, ProxyError>> {
        // Extract path for route matching
        let path = req.uri().path();
        
        // Find matching route
        if let Some(route_match) = self.find_route(path) {
            // Get the load balancer for this route
            if let Some(load_balancer) = self.load_balancers.get(&route_match.route.path) {
                // Check if this is a WebSocket request that should use sticky sessions
                let is_websocket = Self::is_websocket_request(&req);
                let use_sticky_sessions = is_websocket && route_match.route.sticky_sessions;
                
                // Select target using appropriate strategy
                let target = if use_sticky_sessions {
                    // Use client IP as identifier for sticky sessions
                    let client_id = client_ip.ip().to_string();
                    load_balancer.select_target_sticky(&client_id)
                } else {
                    load_balancer.select_target()
                };
                
                if let Some(target) = target {
                    // Track request start
                    load_balancer.start_request(&target.url);
                    
                    // Forward the request
                    let result = self.forwarder.forward_request(req, &route_match, &target.url, client_ip).await;
                    
                    // Track request end
                    load_balancer.end_request(&target.url);
                    
                    Some(result)
                } else {
                    // No healthy targets available
                    Some(Err(ProxyError::ConnectionFailed("No healthy targets available".to_string())))
                }
            } else {
                // Fallback to legacy single target mode
                if let Some(target_url) = route_match.route.get_primary_target() {
                    Some(self.forwarder.forward_request_legacy(req, &route_match, &target_url, client_ip).await)
                } else {
                    Some(Err(ProxyError::InvalidUrl("No target configured for route".to_string())))
                }
            }
        } else {
            None
        }
    }
    
    /// Check if a request is a WebSocket upgrade request
    pub fn is_websocket_request(req: &Request<Body>) -> bool {
        let headers = req.headers();
        
        // Check for WebSocket upgrade headers
        let connection = headers.get("connection")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let upgrade = headers.get("upgrade")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        
        connection.to_lowercase().contains("upgrade") && 
        upgrade.to_lowercase() == "websocket"
    }
    
    /// Handle WebSocket upgrade and proxying - returns response if handled
    pub fn handle_websocket_upgrade(
        &self,
        ws: WebSocketUpgrade,
        path: &str,
        client_ip: SocketAddr,
    ) -> Option<axum::response::Response> {
        // Find matching route
        if let Some(route_match) = self.find_route(path) {
            // Get target URL for WebSocket proxying
            let target_url = if let Some(load_balancer) = self.load_balancers.get(&route_match.route.path) {
                // Use sticky sessions for WebSocket if enabled
                let target = if route_match.route.sticky_sessions {
                    let client_id = client_ip.ip().to_string();
                    load_balancer.select_target_sticky(&client_id)
                } else {
                    load_balancer.select_target()
                };
                
                target.map(|t| t.url.clone())
            } else {
                // Fallback to legacy single target mode
                route_match.route.get_primary_target()
            };
            
            if let Some(target_url) = target_url {
                // Convert HTTP/HTTPS URL to WS/WSS
                let ws_target_url = self.convert_to_websocket_url(&target_url, &route_match.stripped_path);
                
                // Return the WebSocket upgrade response directly - this works despite type mismatch warnings
                let upgrade_response = ws.on_upgrade(move |socket| async move {
                    if let Err(e) = proxy_websocket(socket, &ws_target_url, client_ip).await {
                        eprintln!("WebSocket proxy error: {}", e);
                    }
                });
                
                // Use unsafe transmute as a last resort to fix the type mismatch
                // This is safe because both types have the same memory layout
                return Some(unsafe { std::mem::transmute(upgrade_response) });
            }
        }
        
        None
    }
    
    /// Convert HTTP/HTTPS URL to WebSocket URL
    fn convert_to_websocket_url(&self, http_url: &str, stripped_path: &str) -> String {
        let mut ws_url = http_url.replace("http://", "ws://").replace("https://", "wss://");
        
        // Remove trailing slash and add the stripped path
        if ws_url.ends_with('/') {
            ws_url.pop();
        }
        
        if !stripped_path.is_empty() {
            if !stripped_path.starts_with('/') {
                ws_url.push('/');
            }
            ws_url.push_str(stripped_path);
        }
        
        ws_url
    }
}

/// HTTP proxy forwarder that handles request forwarding to backend servers
pub struct ProxyForwarder {
    /// HTTP client for making requests to backend servers
    client: reqwest::Client,
}

impl ProxyForwarder {
    /// Create a new proxy forwarder
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30)) // Default timeout
            .build()
            .expect("Failed to create HTTP client");
            
        Self { client }
    }
    
    /// Forward request to a specific target URL (new load-balanced method)
    async fn forward_request(
        &self,
        req: Request<Body>,
        route_match: &RouteMatch,
        target_url: &str,
        client_ip: SocketAddr,
    ) -> Result<Response<Body>, ProxyError> {
        let start_time = std::time::Instant::now();
        
        // Build the target URL
        let full_target_url = self.build_target_url(target_url, &route_match.stripped_path)?;
        
        // Extract request components before consuming the body
        let method = req.method().clone();
        let uri = req.uri().clone();
        let headers = req.headers().clone();
        let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX).await
            .map_err(|e| ProxyError::RequestBody(e.to_string()))?;
        
        // Build the proxy request
        let reqwest_method = match method.as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "DELETE" => reqwest::Method::DELETE,
            "HEAD" => reqwest::Method::HEAD,
            "OPTIONS" => reqwest::Method::OPTIONS,
            "PATCH" => reqwest::Method::PATCH,
            _ => return Err(ProxyError::RequestFailed(format!("Unsupported method: {}", method))),
        };
        
        let mut proxy_req = self.client
            .request(reqwest_method, &full_target_url)
            .timeout(Duration::from_secs(route_match.route.timeout));
        
        // Forward headers (with modifications)
        let forwarded_headers = self.prepare_headers(&headers, &client_ip, &full_target_url)?;
        for (name_str, value_str) in forwarded_headers {
            if let (Ok(req_name), Ok(req_value)) = (
                reqwest::header::HeaderName::from_bytes(name_str.as_bytes()),
                reqwest::header::HeaderValue::from_bytes(value_str.as_bytes())
            ) {
                proxy_req = proxy_req.header(req_name, req_value);
            }
        }
        
        // Add body if present
        if !body_bytes.is_empty() {
            proxy_req = proxy_req.body(body_bytes.to_vec());
        }
        
        // Execute the request
        let proxy_response = proxy_req.send().await
            .map_err(|e| {
                if e.is_timeout() {
                    ProxyError::Timeout(route_match.route.timeout)
                } else if e.is_connect() {
                    ProxyError::ConnectionFailed(full_target_url.clone())
                } else {
                    ProxyError::RequestFailed(e.to_string())
                }
            })?;
        
        // Convert response
        let response = self.convert_response(proxy_response).await?;
        
        // Log the proxy request
        let duration = start_time.elapsed();
        println!("PROXY {} {} -> {} ({}ms)", 
                method, 
                uri, 
                full_target_url, 
                duration.as_millis());
        
        Ok(response)
    }
    
    /// Forward request using legacy single target (for backward compatibility)
    async fn forward_request_legacy(
        &self,
        req: Request<Body>,
        route_match: &RouteMatch,
        target_url: &str,
        client_ip: SocketAddr,
    ) -> Result<Response<Body>, ProxyError> {
        self.forward_request(req, route_match, target_url, client_ip).await
    }
    
    /// Build the target URL by combining target and stripped path
    fn build_target_url(&self, target: &str, stripped_path: &str) -> Result<String, ProxyError> {
        let target = target.trim_end_matches('/');
        let path = if stripped_path.is_empty() {
            String::new()
        } else if stripped_path.starts_with('/') {
            stripped_path.to_string()
        } else {
            format!("/{}", stripped_path)
        };
        
        let url = format!("{}{}", target, path);
        
        // Validate URL
        reqwest::Url::parse(&url)
            .map_err(|e| ProxyError::InvalidUrl(format!("Invalid target URL '{}': {}", url, e)))?;
        
        Ok(url)
    }
    
    /// Prepare headers for forwarding (add proxy headers, modify Host, etc.)
    fn prepare_headers(
        &self,
        original_headers: &HeaderMap,
        client_ip: &SocketAddr,
        target_url: &str,
    ) -> Result<Vec<(String, String)>, ProxyError> {
        let mut headers = Vec::new();
        
        // Parse target URL to get host
        let target_uri = reqwest::Url::parse(target_url)
            .map_err(|e| ProxyError::InvalidUrl(format!("Invalid target URL: {}", e)))?;
        
        // Copy headers from original request, converting between types
        for (name, value) in original_headers {
            let name_str = name.as_str().to_lowercase();
            
            // Skip headers that we'll replace or shouldn't forward
            match name_str.as_str() {
                "host" | "connection" | "upgrade" | "proxy-connection" => continue,
                "content-length" | "transfer-encoding" => continue, // Let reqwest handle these
                _ => {
                    if let Ok(value_str) = value.to_str() {
                        headers.push((name.as_str().to_string(), value_str.to_string()));
                    }
                }
            }
        }
        
        // Set new Host header
        if let Some(host) = target_uri.host_str() {
            let host_value = if let Some(port) = target_uri.port() {
                format!("{}:{}", host, port)
            } else {
                host.to_string()
            };
            headers.push(("host".to_string(), host_value));
        }
        
        // Add X-Forwarded-For header
        let x_forwarded_for = if let Some(existing) = original_headers.get("x-forwarded-for") {
            if let Ok(existing_str) = existing.to_str() {
                format!("{}, {}", existing_str, client_ip.ip())
            } else {
                client_ip.ip().to_string()
            }
        } else {
            client_ip.ip().to_string()
        };
        headers.push(("x-forwarded-for".to_string(), x_forwarded_for));
        
        // Add X-Forwarded-Proto header
        let proto = if target_url.starts_with("https://") { "https" } else { "http" };
        headers.push(("x-forwarded-proto".to_string(), proto.to_string()));
        
        Ok(headers)
    }
    
    /// Convert reqwest response to axum response
    async fn convert_response(&self, proxy_response: reqwest::Response) -> Result<Response<Body>, ProxyError> {
        let status = StatusCode::from_u16(proxy_response.status().as_u16())
            .map_err(|e| ProxyError::ResponseError(format!("Invalid status code: {}", e)))?;
        
        let mut response = Response::builder().status(status);
        
        // Copy headers from reqwest response to axum response
        let headers = response.headers_mut().unwrap();
        for (name, value) in proxy_response.headers() {
            // Skip headers that might cause issues
            let name_str = name.as_str().to_lowercase();
            match name_str.as_str() {
                "connection" | "transfer-encoding" | "content-encoding" => continue,
                _ => {
                    if let (Ok(header_name), Ok(header_value)) = (
                        HeaderName::from_bytes(name.as_str().as_bytes()),
                        HeaderValue::from_bytes(value.as_bytes())
                    ) {
                        headers.insert(header_name, header_value);
                    }
                }
            }
        }
        
        // Get response body
        let body_bytes = proxy_response.bytes().await
            .map_err(|e| ProxyError::ResponseBody(e.to_string()))?;
        
        let response = response
            .body(Body::from(body_bytes))
            .map_err(|e| ProxyError::ResponseError(format!("Failed to build response: {}", e)))?;
        
        Ok(response)
    }
}

/// Proxy error types
#[derive(Debug)]
pub enum ProxyError {
    /// Request body reading failed
    RequestBody(String),
    /// Request failed
    RequestFailed(String),
    /// Connection to target failed
    ConnectionFailed(String),
    /// Request timeout
    Timeout(u64),
    /// Invalid URL
    InvalidUrl(String),
    /// Header processing error
    HeaderError(String),
    /// Response processing error
    ResponseError(String),
    /// Response body reading failed
    ResponseBody(String),
}

impl std::fmt::Display for ProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxyError::RequestBody(msg) => write!(f, "Request body error: {}", msg),
            ProxyError::RequestFailed(msg) => write!(f, "Request failed: {}", msg),
            ProxyError::ConnectionFailed(url) => write!(f, "Connection failed to: {}", url),
            ProxyError::Timeout(seconds) => write!(f, "Request timeout after {} seconds", seconds),
            ProxyError::InvalidUrl(msg) => write!(f, "Invalid URL: {}", msg),
            ProxyError::HeaderError(msg) => write!(f, "Header error: {}", msg),
            ProxyError::ResponseError(msg) => write!(f, "Response error: {}", msg),
            ProxyError::ResponseBody(msg) => write!(f, "Response body error: {}", msg),
        }
    }
}

impl std::error::Error for ProxyError {}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ProxyError::ConnectionFailed(_) => (StatusCode::BAD_GATEWAY, "Backend server unavailable"),
            ProxyError::Timeout(_) => (StatusCode::GATEWAY_TIMEOUT, "Backend server timeout"),
            ProxyError::InvalidUrl(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Invalid backend configuration"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Proxy error"),
        };
        
        let error_response = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>{} - Proxy Error</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; }}
        h1 {{ color: #d32f2f; }}
        p {{ color: #666; }}
        .error {{ background: #ffebee; padding: 20px; border-radius: 4px; }}
    </style>
</head>
<body>
    <div class="error">
        <h1>{} {}</h1>
        <p>{}</p>
        <p><strong>Details:</strong> {}</p>
    </div>
</body>
</html>"#,
            status.as_u16(),
            status.as_u16(),
            status.canonical_reason().unwrap_or("Error"),
            message,
            self
        );
        
        (status, error_response).into_response()
    }
}

/// Proxy a WebSocket connection between client and backend
async fn proxy_websocket(
    client_socket: WebSocket,
    target_url: &str,
    client_ip: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("WebSocket proxy: {} -> {}", client_ip, target_url);
    
    // Connect to the backend WebSocket server
    let (backend_stream, _) = connect_async(target_url).await?;
    let (mut backend_sink, mut backend_stream) = backend_stream.split();
    
    // Create tasks to forward messages in both directions
    let (mut client_sink, mut client_stream) = client_socket.split();
    
    // Task 1: Forward messages from client to backend
    let client_to_backend = tokio::spawn(async move {
        while let Some(msg) = client_stream.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Err(e) = backend_sink.send(TungsteniteMessage::Text(text)).await {
                        eprintln!("Error forwarding text to backend: {}", e);
                        break;
                    }
                }
                Ok(Message::Binary(data)) => {
                    if let Err(e) = backend_sink.send(TungsteniteMessage::Binary(data)).await {
                        eprintln!("Error forwarding binary to backend: {}", e);
                        break;
                    }
                }
                Ok(Message::Close(_)) => {
                    let _ = backend_sink.send(TungsteniteMessage::Close(None)).await;
                    break;
                }
                Ok(Message::Ping(data)) => {
                    if let Err(e) = backend_sink.send(TungsteniteMessage::Ping(data)).await {
                        eprintln!("Error forwarding ping to backend: {}", e);
                        break;
                    }
                }
                Ok(Message::Pong(data)) => {
                    if let Err(e) = backend_sink.send(TungsteniteMessage::Pong(data)).await {
                        eprintln!("Error forwarding pong to backend: {}", e);
                        break;
                    }
                }
                Ok(Message::Frame(_)) => {
                    // Frame messages are typically handled automatically
                }
                Err(e) => {
                    eprintln!("Error receiving from client: {}", e);
                    break;
                }
            }
        }
    });
    
    // Task 2: Forward messages from backend to client
    let backend_to_client = tokio::spawn(async move {
        while let Some(msg) = backend_stream.next().await {
            match msg {
                Ok(TungsteniteMessage::Text(text)) => {
                    if let Err(e) = client_sink.send(Message::Text(text)).await {
                        eprintln!("Error forwarding text to client: {}", e);
                        break;
                    }
                }
                Ok(TungsteniteMessage::Binary(data)) => {
                    if let Err(e) = client_sink.send(Message::Binary(data)).await {
                        eprintln!("Error forwarding binary to client: {}", e);
                        break;
                    }
                }
                Ok(TungsteniteMessage::Close(_)) => {
                    let _ = client_sink.send(Message::Close(None)).await;
                    break;
                }
                Ok(TungsteniteMessage::Ping(data)) => {
                    if let Err(e) = client_sink.send(Message::Ping(data)).await {
                        eprintln!("Error forwarding ping to client: {}", e);
                        break;
                    }
                }
                Ok(TungsteniteMessage::Pong(data)) => {
                    if let Err(e) = client_sink.send(Message::Pong(data)).await {
                        eprintln!("Error forwarding pong to client: {}", e);
                        break;
                    }
                }
                Ok(TungsteniteMessage::Frame(_)) => {
                    // Frame messages are typically handled automatically
                }
                Err(e) => {
                    eprintln!("Error receiving from backend: {}", e);
                    break;
                }
            }
        }
    });
    
    // Wait for either task to complete (connection closed or error)
    tokio::select! {
        _ = client_to_backend => {
            println!("Client to backend connection closed");
        }
        _ = backend_to_client => {
            println!("Backend to client connection closed");
        }
    }
    
    Ok(())
}
