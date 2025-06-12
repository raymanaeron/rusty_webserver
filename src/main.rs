use axum::{
    extract::{ConnectInfo, Path, Request},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use chrono::Utc;
use clap::Parser;
use mime_guess::from_path;
use std::{net::SocketAddr, path::PathBuf};
use tokio::fs;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

#[derive(Parser)]
#[command(name = "httpserver")]
#[command(about = "A simple cross-platform HTTP server")]
struct Args {
    /// Directory to serve files from
    #[arg(short, long, default_value = ".")]
    directory: PathBuf,

    /// Port to listen on
    #[arg(short, long, default_value = "8080")]
    port: u16,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Resolve the directory to an absolute path
    let serve_dir = match args.directory.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: Cannot access directory '{}': {}", args.directory.display(), e);
            std::process::exit(1);
        }
    };

    println!("Starting HTTP server on port {}", args.port);
    println!("Serving files from: {}", serve_dir.display());

    // Create the router with CORS support and logging middleware
    let serve_dir_clone = serve_dir.clone();
    let app = Router::new()
        .route("/", get({
            let serve_dir = serve_dir.clone();
            move || serve_file("index.html".to_string(), serve_dir)
        }))
        .route("/*path", get(move |Path(path): Path<String>| serve_file(path, serve_dir_clone)))
        .layer(
            ServiceBuilder::new()
                .layer(middleware::from_fn(logging_middleware))
                .layer(CorsLayer::permissive()) // Allow all CORS requests
        );

    // Start the server
    let listener = match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", args.port)).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("Error: Failed to bind to port {}: {}", args.port, e);
            std::process::exit(1);
        }
    };

    println!("Server running at http://localhost:{}", args.port);

    if let Err(e) = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}

async fn logging_middleware(
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

async fn serve_file(path: String, base_dir: PathBuf) -> impl IntoResponse {
    // Clean up the path and prevent directory traversal
    let requested_path = if path.is_empty() || path == "/" {
        "index.html".to_string()
    } else {
        path
    };

    // Remove leading slash if present
    let clean_path = requested_path.strip_prefix('/').unwrap_or(&requested_path);
    
    // Build the full file path
    let file_path = base_dir.join(clean_path);

    // Security check: ensure the resolved path is within the base directory
    match file_path.canonicalize() {
        Ok(canonical_path) => {
            if !canonical_path.starts_with(&base_dir) {
                return create_error_response(StatusCode::FORBIDDEN, "Access denied");
            }
        }
        Err(_) => {
            // File doesn't exist, we'll handle this below
        }
    }

    // Try to read the file
    match fs::read(&file_path).await {
        Ok(contents) => {
            // Guess the MIME type based on file extension
            let mime_type = from_path(&file_path).first_or_octet_stream();
            
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime_type.as_ref())
                .header(header::CACHE_CONTROL, "public, max-age=3600")
                .body(contents.into())
                .unwrap()
        }
        Err(_) => {
            // If the requested file doesn't exist, try to serve index.html for SPA support
            if clean_path != "index.html" {
                let index_path = base_dir.join("index.html");
                if let Ok(contents) = fs::read(&index_path).await {
                    return Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, "text/html")
                        .body(contents.into())
                        .unwrap();
                }
            }

            create_error_response(StatusCode::NOT_FOUND, "File not found")
        }
    }
}

fn create_error_response(status: StatusCode, message: &str) -> Response {
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
        .header(header::CONTENT_TYPE, "text/html")
        .body(html.into())
        .unwrap()
}
