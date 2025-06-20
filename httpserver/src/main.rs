use clap::Parser;
use httpserver_config::{ Args, Config, create_config_health_router };
use httpserver_core::{
    Server,
    create_health_router,
    initialize_logging,
    cleanup_old_logs,
    SslCertificateManager,
};
use httpserver_static::{ StaticHandler, create_static_health_router };
use httpserver_proxy::ProxyHandler;
use httpserver_balancer::create_balancer_health_router;
use httpserver_tunnel::server::TunnelServer;
use axum::{
    Router,
    extract::{ Request, ConnectInfo },
    response::{ IntoResponse },
    middleware::{ self, Next },
    http::StatusCode,
};
use std::sync::Arc;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args = Args::parse();
    let port = args.port;

    // Load configuration: if --config is specified, load only that file and bypass app_config.toml
    let config = if let Some(config_path) = &args.config {
        // When --config is specified, load only that file (don't load app_config.toml)
        tracing::info!(config_file = %config_path.display(), "Loading configuration from CLI-specified file, bypassing app_config.toml");
        let mut config = Config::load_from_file(config_path)?;
        // Override static directory with CLI argument
        config.static_config.directory = args.directory;
        config
    } else {
        // No --config specified, try to load app_config.toml or use defaults
        let mut config = Config::load_app_config()?;
        // Override static directory with CLI argument
        config.static_config.directory = args.directory;
        config
    };

    // Initialize logging system with app config
    initialize_logging(&config.logging)?;

    // Clean up old log files
    if let Err(e) = cleanup_old_logs(&config.logging) {
        tracing::warn!(error = %e, "Failed to clean up old log files");
    }

    tracing::info!("Application starting");

    // Create the static file handler
    let static_handler = StaticHandler::new(config.static_config.directory.clone())?;

    // Create the proxy handler
    let proxy_handler = ProxyHandler::new(config.proxy.clone());

    // Initialize SSL if configured
    let mut ssl_cert_manager = SslCertificateManager::new();
    let ssl_server_config = if let Some(ssl_config) = &config.server.ssl {
        if ssl_config.enabled {
            tracing::info!("SSL/TLS enabled, loading certificates");

            // Load wildcard certificate if configured
            if let Some(wildcard_config) = &ssl_config.wildcard {
                ssl_cert_manager.load_certificate_from_files(
                    wildcard_config.domain.clone(),
                    &wildcard_config.cert_file,
                    &wildcard_config.key_file,
                    None
                )?;
                tracing::info!(domain = %wildcard_config.domain, "Wildcard certificate loaded");
            }

            // Load main certificate if specified
            if
                let (Some(cert_file), Some(key_file)) = (
                    &ssl_config.cert_file,
                    &ssl_config.key_file,
                )
            {
                let domain = "localhost".to_string(); // Default domain
                ssl_cert_manager.load_certificate_from_files(
                    domain.clone(),
                    cert_file,
                    key_file,
                    ssl_config.cert_chain_file.as_ref()
                )?;
                tracing::info!(domain = %domain, "Main certificate loaded");
            }

            // Create SSL server config
            if ssl_cert_manager.has_certificates() {
                let domain = ssl_cert_manager
                    .get_wildcard_domain()
                    .unwrap_or_else(|| "localhost".to_string());
                Some(ssl_cert_manager.create_server_config(&domain)?)
            } else {
                tracing::warn!("SSL enabled but no certificates loaded");
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Initialize tunnel server if configured
    let tunnel_server = if config.tunnel.enabled {
        tracing::info!("Tunnel server enabled, initializing");
        let server = TunnelServer::new(config.tunnel.server.clone())?;
        
        // Start tunnel server in background
        let tunnel_handle = {
            let server = server;
            tokio::spawn(async move {
                if let Err(e) = server.start().await {
                    tracing::error!("Tunnel server error: {}", e);
                }
            })
        };
        
        tracing::info!(
            port = config.tunnel.server.tunnel_port,
            base_domain = %config.tunnel.server.base_domain,
            "Tunnel server started"
        );
        
        Some(tunnel_handle)
    } else {
        tracing::info!("Tunnel server disabled");
        None
    };

    // Create the router with proxy routes taking precedence over static files
    let app = create_router(proxy_handler, static_handler, &config).await?;

    // Start the server with SSL support if configured
    let server = if let Some(ssl_config_arc) = ssl_server_config {
        let ssl_config = config.server.ssl.as_ref().unwrap();
        Server::new_with_ssl(port, ssl_config_arc, ssl_config.https_port)
    } else {
        Server::new(port)
    };

    // Start main server and tunnel server concurrently
    if let Some(tunnel_handle) = tunnel_server {
        tracing::info!("Starting main HTTP server and tunnel server concurrently");
        
        let main_server_future = server.start(app);
        
        // Wait for either server to complete (or error)
        tokio::select! {
            result = main_server_future => {
                tracing::info!("Main server completed");
                result?;
            }
            result = tunnel_handle => {
                tracing::info!("Tunnel server completed");
                if let Err(e) = result {
                    tracing::error!("Tunnel server task error: {}", e);
                    return Err(e.into());
                }
            }
        }
    } else {
        // No tunnel server, just start main server
        tracing::info!("Starting main HTTP server only");
        server.start(app).await?;
    }

    Ok(())
}

/// Create the main router with proxy routes having priority over static files
async fn create_router(
    proxy_handler: ProxyHandler,
    static_handler: StaticHandler,
    _config: &Config
) -> Result<Router, Box<dyn std::error::Error>> {
    // Start with the static file router
    let static_router = static_handler.create_router();

    // Add gateway health endpoints with highest priority
    let health_router = create_health_router();

    // Add service-specific health endpoints
    let config_health_router = create_config_health_router();
    let static_health_router = create_static_health_router();
    let balancer_health_router = create_balancer_health_router();

    // If proxy routes are configured, add proxy middleware with higher priority
    if proxy_handler.has_routes() {
        tracing::info!(route_count = proxy_handler.routes().len(), "Proxy routes configured");
        for route in proxy_handler.routes() {
            let targets = route.get_targets();
            if targets.len() > 1 {
                tracing::info!(
                    path = %route.path,
                    target_count = targets.len(),
                    strategy = %route.strategy,
                    "Multi-target route configured"
                );
                for (i, target) in targets.iter().enumerate() {
                    tracing::debug!(
                        route = %route.path,
                        target_index = i + 1,
                        url = %target.url,
                        weight = target.weight,
                        "Route target configured"
                    );
                }
            } else if let Some(target_url) = route.get_primary_target() {
                tracing::info!(
                    path = %route.path,
                    target = %target_url,
                    "Single-target route configured"
                );
            }
        }

        // Wrap proxy handler in Arc for sharing across requests
        let proxy_handler = Arc::new(proxy_handler);

        // Create router with proxy middleware that runs before static file serving
        let app = static_router
            .merge(health_router)
            .merge(config_health_router)
            .merge(static_health_router)
            .merge(balancer_health_router)
            .layer(middleware::from_fn_with_state(proxy_handler, proxy_middleware));

        tracing::info!("Proxy forwarding active - routes will be processed before static files");
        tracing::info!(
            "Health endpoints available: /health, /ping, /config/health, /static/health, /balancer/health"
        );
        Ok(app)
    } else {
        // No proxy routes, just return static router with health endpoints
        let app = static_router
            .merge(health_router)
            .merge(config_health_router)
            .merge(static_health_router)
            .merge(balancer_health_router);

        tracing::info!(
            "Health endpoints available: /health, /ping, /config/health, /static/health, /balancer/health"
        );
        Ok(app)
    }
}

/// Middleware that handles proxy requests before they reach static file serving
async fn proxy_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::extract::State(state): axum::extract::State<Arc<ProxyHandler>>,
    req: Request,
    next: Next
) -> axum::response::Response {
    // Check if this request matches any proxy routes
    let path = req.uri().path().to_string();

    if let Some(_route_match) = state.find_route(&path) {
        // For now, WebSocket support is implemented but requires dedicated routing
        // This middleware handles HTTP requests only
        match state.handle_request(req, addr).await {
            Some(Ok(response)) => response.into_response(),
            Some(Err(proxy_error)) => proxy_error.into_response(),
            None => {
                // This shouldn't happen since we found a route, but handle gracefully
                (StatusCode::INTERNAL_SERVER_ERROR, "Proxy routing error").into_response()
            }
        }
    } else {
        // No proxy route matched, continue to static file serving
        next.run(req).await
    }
}
