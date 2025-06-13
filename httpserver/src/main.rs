use clap::Parser;
use httpserver_config::{Args, Config, create_config_health_router};
use httpserver_core::{Server, create_health_router, initialize_logging, cleanup_old_logs};
use httpserver_static::{StaticHandler, create_static_health_router};
use httpserver_proxy::ProxyHandler;
use httpserver_balancer::create_balancer_health_router;
use axum::{
    Router, 
    extract::{Request, ConnectInfo},
    response::{IntoResponse},
    middleware::{self, Next},
    http::StatusCode,
};
use std::sync::Arc;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args = Args::parse();
    let port = args.port;
    
    // Create configuration from arguments
    let config = Config::from_args(args)?;
    
    // Initialize logging system
    initialize_logging(&config.logging)?;
    
    // Clean up old log files
    if let Err(e) = cleanup_old_logs(&config.logging) {
        tracing::warn!(error = %e, "Failed to clean up old log files");
    }
    
    tracing::info!("Application starting");
    
    // Create the static file handler
    let static_handler = StaticHandler::new(config.static_config.directory)?;
    
    // Create the proxy handler
    let proxy_handler = ProxyHandler::new(config.proxy.clone());
    
    // Create the router with proxy routes taking precedence over static files
    let app = create_router(proxy_handler, static_handler).await?;
    
    // Start the server (middleware will be applied inside)
    let server = Server::new(port);
    server.start(app).await?;
    
    Ok(())
}

/// Create the main router with proxy routes having priority over static files
async fn create_router(
    proxy_handler: ProxyHandler,
    static_handler: StaticHandler,
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
            .layer(middleware::from_fn_with_state(
                proxy_handler,
                proxy_middleware
            ));
        
        tracing::info!("Proxy forwarding active - routes will be processed before static files");
        tracing::info!("Health endpoints available: /health, /ping, /config/health, /static/health, /balancer/health");
        Ok(app)
    } else {
        // No proxy routes, just return static router with health endpoints
        let app = static_router
            .merge(health_router)
            .merge(config_health_router)
            .merge(static_health_router)
            .merge(balancer_health_router);
        
        tracing::info!("Health endpoints available: /health, /ping, /config/health, /static/health, /balancer/health");
        Ok(app)
    }
}

/// Middleware that handles proxy requests before they reach static file serving
async fn proxy_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::extract::State(state): axum::extract::State<Arc<ProxyHandler>>,
    req: Request,
    next: Next,
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
