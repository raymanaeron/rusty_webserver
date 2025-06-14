use axum::{
    extract::{ ConnectInfo, Request },
    http::StatusCode,
    middleware::Next,
    response::{ Response, Json, IntoResponse },
    Router,
    routing::get,
};
use chrono::Utc;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use serde_json::json;
use tracing::{ info, error, instrument, Instrument };
use std::sync::Arc;
use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tower::Service;

// Export logging functionality
pub mod logging;
pub use logging::{ initialize_logging, create_request_span, check_log_rotation, cleanup_old_logs };

// Export SSL functionality
pub mod ssl;
pub use ssl::{ SslCertificateManager, SslCertificate, SslRedirectConfig };

/// Core server functionality
pub struct Server {
    pub port: u16,
    pub ssl_config: Option<Arc<rustls::ServerConfig>>,
    pub https_port: Option<u16>,
}

impl Server {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            ssl_config: None,
            https_port: None,
        }
    }

    /// Create server with SSL configuration
    pub fn new_with_ssl(port: u16, ssl_config: Arc<rustls::ServerConfig>, https_port: u16) -> Self {
        Self {
            port,
            ssl_config: Some(ssl_config),
            https_port: Some(https_port),
        }
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

        // Start HTTP server
        let http_task = {
            let app = app.clone();
            let port = self.port;
            tokio::spawn(async move {
                let listener = match
                    tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await
                {
                    Ok(listener) => listener,
                    Err(e) => {
                        error!(port = port, error = %e, "Failed to bind to HTTP port");
                        return Err(e.into());
                    }
                };

                info!(port = port, "HTTP server running at http://localhost:{}", port);

                if
                    let Err(e) = axum::serve(
                        listener,
                        app.into_make_service_with_connect_info::<SocketAddr>()
                    ).await
                {
                    error!(error = %e, "HTTP server error");
                    return Err(e.into());
                }
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            })
        };

        // Start HTTPS server if SSL is configured
        let https_task = if
            let (Some(ssl_config), Some(https_port)) = (self.ssl_config, self.https_port)
        {
            let app = app.clone();
            Some(
                tokio::spawn(async move {
                    let listener = match
                        TcpListener::bind(format!("0.0.0.0:{}", https_port)).await
                    {
                        Ok(listener) => listener,
                        Err(e) => {
                            error!(port = https_port, error = %e, "Failed to bind to HTTPS port");
                            return Err(e.into());
                        }
                    };

                    info!(
                        port = https_port,
                        "HTTPS server running at https://localhost:{}",
                        https_port
                    );

                    let tls_acceptor = TlsAcceptor::from(ssl_config);
                    let service = app.into_make_service_with_connect_info::<SocketAddr>();

                    loop {
                        let (tcp_stream, remote_addr) = match listener.accept().await {
                            Ok(conn) => conn,
                            Err(e) => {
                                error!(error = %e, "Failed to accept HTTPS connection");
                                continue;
                            }
                        };

                        let tls_acceptor = tls_acceptor.clone();
                        let mut service = service.clone();

                        tokio::spawn(async move {
                            // Perform TLS handshake
                            let tls_stream = match tls_acceptor.accept(tcp_stream).await {
                                Ok(tls_stream) => tls_stream,
                                Err(e) => {
                                    error!(error = %e, remote_addr = %remote_addr, "TLS handshake failed");
                                    return;
                                }
                            };

                            let io = TokioIo::new(tls_stream);
                            let hyper_service = match service.call(remote_addr).await {
                                Ok(service) => service,
                                Err(e) => {
                                    error!(error = %e, "Failed to create service");
                                    return;
                                }
                            };

                            // Wrap the axum service for hyper compatibility
                            let hyper_service = TowerToHyperService::new(hyper_service);

                            // Serve the connection using HTTP/1.1
                            if let Err(e) = http1::Builder::new()
                                .serve_connection(io, hyper_service)
                                .await
                            {
                                error!(error = %e, remote_addr = %remote_addr, "HTTPS connection error");
                            }
                        });
                    }

                    #[allow(unreachable_code)]
                    Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
                })
            )
        } else {
            None
        };

        // Wait for either server to complete (or fail)
        if let Some(https_task) = https_task {
            tokio::select! {
                result = http_task => {
                    if let Err(e) = result? {
                        error!(error = %e, "HTTP server failed");
                        return Err(e);
                    }
                }
                result = https_task => {
                    if let Err(e) = result? {
                        error!(error = %e, "HTTPS server failed");
                        return Err(e);
                    }
                }
            }
        } else {
            // Only HTTP server
            if let Err(e) = http_task.await? {
                error!(error = %e, "HTTP server failed");
                return Err(e);
            }
        }

        Ok(())
    }
}

/// Logging middleware that captures all requests
pub async fn logging_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next
) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path();
    let client_ip = addr.ip().to_string();

    // Create request span for tracing
    let span = create_request_span(&method.to_string(), path, &client_ip);

    (
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
    ).instrument(span).await
}

/// Gateway health endpoint handler
pub async fn gateway_health() -> Json<serde_json::Value> {
    Json(
        json!({
        "status": "healthy",
        "service": "httpserver-gateway",
        "timestamp": Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION")
    })
    )
}

/// Create gateway health endpoint router
pub fn create_health_router() -> Router {
    Router::new().route("/health", get(gateway_health)).route("/ping", get(gateway_health))
}

/// Create a standard error response
pub fn create_error_response(status: StatusCode, message: &str) -> Response {
    (status, message.to_string()).into_response()
}

/// HTTPS redirect middleware
pub async fn https_redirect_middleware(req: Request, next: Next) -> Response {
    use axum::http::{ header, StatusCode, Uri };

    // Check if request is already HTTPS
    if req.uri().scheme_str() == Some("https") {
        return next.run(req).await;
    }

    // Check if this is a health check or other exempt path
    let path = req.uri().path();
    if path.starts_with("/health") || path.starts_with("/ping") {
        return next.run(req).await;
    }

    // Get the host header
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");

    // Remove port from host if present, then add HTTPS port
    let host_without_port = host.split(':').next().unwrap_or(host);
    let https_host = if host_without_port == "localhost" || host_without_port.starts_with("127.") {
        format!("{}:443", host_without_port)
    } else {
        host_without_port.to_string() // Assume standard HTTPS port 443
    };

    // Construct HTTPS URL
    let https_uri = format!(
        "https://{}{}",
        https_host,
        req
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("")
    );

    // Parse the URI
    match https_uri.parse::<Uri>() {
        Ok(uri) => {
            tracing::info!(
                original_uri = %req.uri(),
                redirect_uri = %uri,
                "Redirecting HTTP to HTTPS"
            );

            Response::builder()
                .status(StatusCode::MOVED_PERMANENTLY)
                .header(header::LOCATION, uri.to_string())
                .body("Redirecting to HTTPS".into())
                .unwrap_or_else(|_|
                    create_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to create redirect response"
                    )
                )
        }
        Err(e) => {
            tracing::error!(
                error = %e,
                https_uri = %https_uri,
                "Failed to parse HTTPS redirect URI"
            );
            next.run(req).await
        }
    }
}
