use clap::Parser;
use httpserver_config::{Args, Config};
use httpserver_core::Server;
use httpserver_static::StaticHandler;
use httpserver_proxy::ProxyHandler;
use axum::{
    Router, 
    extract::{Request, ConnectInfo},
    response::{Response, IntoResponse},
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
    
    // If proxy routes are configured, add proxy middleware with higher priority
    if proxy_handler.has_routes() {
        println!("Proxy routes configured: {}", proxy_handler.routes().len());
        for route in proxy_handler.routes() {
            let targets = route.get_targets();
            if targets.len() > 1 {
                println!("  {} -> {} targets ({})", route.path, targets.len(), route.strategy);
                for (i, target) in targets.iter().enumerate() {
                    println!("    {}. {} (weight: {})", i + 1, target.url, target.weight);
                }
            } else if let Some(target_url) = route.get_primary_target() {
                println!("  {} -> {}", route.path, target_url);
            }
        }
        
        // Wrap proxy handler in Arc for sharing across requests
        let proxy_handler = Arc::new(proxy_handler);
        
        // Create router with proxy middleware that runs before static file serving
        let app = static_router
            .layer(middleware::from_fn_with_state(
                proxy_handler,
                proxy_middleware
            ));
        
        println!("Proxy forwarding active - routes will be processed before static files");
        Ok(app)
    } else {
        // No proxy routes, just return static router
        Ok(static_router)
    }
}

/// Middleware that handles proxy requests before they reach static file serving
async fn proxy_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::extract::State(state): axum::extract::State<Arc<ProxyHandler>>,
    req: Request,
    next: Next,
) -> Response {
    // Check if this request matches any proxy routes
    let path = req.uri().path().to_string();
    
    if let Some(_route_match) = state.find_route(&path) {
        // This is a proxy request - handle it
        match state.handle_request(req, addr).await {
            Some(Ok(response)) => response,
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
