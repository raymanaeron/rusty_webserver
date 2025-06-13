use axum::{
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::{Response, Json},
    Router,
    routing::get,
};
use chrono::Utc;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use serde_json::json;
use tracing::{info, error, instrument, Instrument};

// Export logging functionality
pub mod logging;
pub use logging::{initialize_logging, create_request_span, check_log_rotation, cleanup_old_logs};

/// Core server functionality
pub struct Server {
    pub port: u16,
}

impl Server {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    /// Start the HTTP server with the given router
    #[instrument(skip(self, app), fields(port = self.port))]
    pub async fn start(self, app: Router) -> Result<(), Box<dyn std::error::Error>> {
        info!(port = self.port, "Starting HTTP server");

        // Apply middleware to the router
        let app = app.layer(
            ServiceBuilder::new()
                .layer(axum::middleware::from_fn(logging_middleware))
                .layer(CorsLayer::permissive())
        );

        let listener = match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.port)).await {
            Ok(listener) => listener,
            Err(e) => {
                error!(port = self.port, error = %e, "Failed to bind to port");
                std::process::exit(1);
            }
        };

        info!(port = self.port, "Server running at http://localhost:{}", self.port);

        if let Err(e) = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await {
            error!(error = %e, "Server error");
            std::process::exit(1);
        }

        Ok(())
    }
}

/// Logging middleware that captures all requests
pub async fn logging_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path();
    let client_ip = addr.ip().to_string();
    
    // Create request span for tracing
    let span = create_request_span(&method.to_string(), path, &client_ip);
    
    async move {
        let start_time = std::time::Instant::now();
        
        // Call the next middleware/handler
        let response = next.run(req).await;
        
        let duration = start_time.elapsed();
        let status = response.status();
        
        // Log request with structured data
        info!(
            method = %method,
            path = %path,
            client_ip = %client_ip,
            status_code = status.as_u16(),
            status_text = status.canonical_reason().unwrap_or("Unknown"),
            duration_ms = duration.as_millis(),
            "Request completed"
        );
        
        response
    }
    .instrument(span)
    .await
}

/// Gateway health endpoint handler
pub async fn gateway_health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "httpserver-gateway",
        "timestamp": Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Create gateway health endpoint router
pub fn create_health_router() -> Router {
    Router::new()
        .route("/health", get(gateway_health))
        .route("/ping", get(gateway_health))
}

/// Create a standard error response
pub fn create_error_response(status: StatusCode, message: &str) -> Response {
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>{} - HTTP Server</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; }}
        h1 {{ color: #333; }}
        p {{ color: #666; }}
    </style>
</head>
<body>
    <h1>{} {}</h1>
    <p>{}</p>
</body>
</html>"#,
        status.as_u16(),
        status.as_u16(),
        status.canonical_reason().unwrap_or("Error"),
        message
    );

    Response::builder()
        .status(status)
        .header(axum::http::header::CONTENT_TYPE, "text/html")
        .body(html.into())
        .unwrap()
}
