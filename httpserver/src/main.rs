use clap::Parser;
use httpserver_config::{Args, Config};
use httpserver_core::Server;
use httpserver_static::StaticHandler;
use httpserver_proxy::ProxyHandler;
use axum::Router;

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
    let app = static_handler.create_router();
    
    // If proxy routes are configured, add them with higher priority
    if proxy_handler.has_routes() {
        // For Phase 2.1, we're just setting up the structure
        // Actual HTTP forwarding will be implemented in Phase 2.2
        println!("Proxy routes configured: {}", proxy_handler.routes().len());
        for route in proxy_handler.routes() {
            println!("  {} -> {}", route.path, route.target);
        }
        println!("Note: HTTP forwarding will be implemented in Phase 2.2");
    }
    
    Ok(app)
}
