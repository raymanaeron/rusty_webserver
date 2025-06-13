use clap::Parser;
use httpserver_config::{Args, Config};
use httpserver_core::Server;
use httpserver_static::StaticHandler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args = Args::parse();
    let port = args.port;
    
    // Create configuration from arguments
    let config = Config::from_args(args)?;
    
    // Create the static file handler
    let static_handler = StaticHandler::new(config.static_config.directory)?;
    
    // Create the router with static file serving
    let app = static_handler.create_router();
    
    // Start the server (middleware will be applied inside)
    let server = Server::new(port);
    server.start(app).await?;
    
    Ok(())
}
