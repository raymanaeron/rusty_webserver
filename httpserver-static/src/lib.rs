use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
    Json,
};
use httpserver_core::create_error_response;
use mime_guess::from_path;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::fs;

/// Static file handler configuration
pub struct StaticHandler {
    pub base_dir: PathBuf,
}

impl StaticHandler {
    /// Create a new static file handler
    pub fn new(base_dir: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        // Resolve the directory to an absolute path
        let resolved_dir = match base_dir.canonicalize() {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Error: Cannot access directory '{}': {}", base_dir.display(), e);
                return Err(e.into());
            }
        };

        println!("Serving files from: {}", resolved_dir.display());

        Ok(Self {
            base_dir: resolved_dir,
        })
    }

    /// Create the router for static file serving
    pub fn create_router(self) -> Router {
        let serve_dir = self.base_dir.clone();
        let serve_dir_clone = self.base_dir.clone();

        Router::new()
            .route("/", get({
                move || serve_file("index.html".to_string(), serve_dir)
            }))
            .route("/*path", get(move |Path(path): Path<String>| {
                serve_file(path, serve_dir_clone)
            }))
    }
}

/// Serve a single file from the static directory
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

/// Health endpoint handler for static file service
pub async fn static_health() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "httpserver-static",
        "message": "Static file serving operational"
    }))
}

/// Create static service health router
pub fn create_static_health_router() -> Router {
    Router::new()
        .route("/static/health", get(static_health))
        .route("/static/status", get(static_health))
}
