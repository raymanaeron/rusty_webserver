use clap::Parser;
use httpserver_config::Args;
use httpserver_engine::HttpServerEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args = Args::parse();

    // Create engine from arguments (preserves exact existing behavior)
    let engine = HttpServerEngine::from_args(args)?;

    // Start the engine
    engine.start().await?;

    Ok(())
}
