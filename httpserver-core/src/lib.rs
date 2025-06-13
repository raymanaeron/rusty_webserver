use axum::{
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::Response,
    Router,
};
use chrono::Utc;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

/// Core server functionality
pub struct Server {
    pub port: u16,
}

impl Server {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    /// Start the HTTP server with the given router
    pub async fn start(self, app: Router) -> Result<(), Box<dyn std::error::Error>> {
        println!("Starting HTTP server on port {}", self.port);

        // Apply middleware to the router
        let app = app.layer(
            ServiceBuilder::new()
                .layer(axum::middleware::from_fn(logging_middleware))
                .layer(CorsLayer::permissive())
        );

        let listener = match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.port)).await {
            Ok(listener) => listener,
            Err(e) => {
                eprintln!("Error: Failed to bind to port {}: {}", self.port, e);
                std::process::exit(1);
            }
        };

        println!("Server running at http://localhost:{}", self.port);

        if let Err(e) = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await {
            eprintln!("Server error: {}", e);
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
    
    // Call the next middleware/handler
    let response = next.run(req).await;
    
    let status = response.status();
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    
    // Log in ngrok-style format: Timestamp | IP | Method URL | Status Code Status Text
    println!(
        "{} | {} | {} {} | {} {}",
        timestamp,
        addr.ip(),
        method,
        uri,
        status.as_u16(),
        status.canonical_reason().unwrap_or("Unknown")
    );
    
    response
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
