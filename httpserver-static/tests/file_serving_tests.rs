use httpserver_static::StaticHandler;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::fs;
use tower::ServiceExt;
use axum::http::{ Request, StatusCode };
use axum::body::to_bytes;

async fn setup_test_files(temp_dir: &TempDir) -> PathBuf {
    let temp_path = temp_dir.path().to_path_buf();

    // Create test files
    fs::write(
        temp_path.join("index.html"),
        "<!DOCTYPE html><html><body><h1>Home Page</h1></body></html>"
    ).await.unwrap();

    fs::write(temp_path.join("test.css"), "body { background-color: #f0f0f0; }").await.unwrap();

    fs::write(temp_path.join("test.js"), "console.log('Hello World');").await.unwrap();

    fs::write(temp_path.join("test.json"), r#"{"message": "test data"}"#).await.unwrap();

    // Create a subdirectory with files
    fs::create_dir(temp_path.join("assets")).await.unwrap();
    fs::write(
        temp_path.join("assets").join("logo.svg"),
        r#"<svg><circle r="10" /></svg>"#
    ).await.unwrap();

    temp_path
}

#[tokio::test]
async fn test_serve_index_html() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = setup_test_files(&temp_dir).await;

    let handler = StaticHandler::new(temp_path).unwrap();
    let app = handler.create_router();

    let request = Request::builder().uri("/").body(axum::body::Body::empty()).unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let content = String::from_utf8(body.to_vec()).unwrap();
    assert!(content.contains("Home Page"));
}

#[tokio::test]
async fn test_serve_css_file() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = setup_test_files(&temp_dir).await;

    let handler = StaticHandler::new(temp_path).unwrap();
    let app = handler.create_router();

    let request = Request::builder().uri("/test.css").body(axum::body::Body::empty()).unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Check content type header
    let content_type = response.headers().get("content-type").unwrap();
    assert!(content_type.to_str().unwrap().contains("text/css"));
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let content = String::from_utf8(body.to_vec()).unwrap();
    assert!(content.contains("background-color"));
}

#[tokio::test]
async fn test_serve_js_file() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = setup_test_files(&temp_dir).await;

    let handler = StaticHandler::new(temp_path).unwrap();
    let app = handler.create_router();

    let request = Request::builder().uri("/test.js").body(axum::body::Body::empty()).unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response.headers().get("content-type").unwrap();
    assert!(content_type.to_str().unwrap().contains("application/javascript"));
}

#[tokio::test]
async fn test_serve_json_file() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = setup_test_files(&temp_dir).await;

    let handler = StaticHandler::new(temp_path).unwrap();
    let app = handler.create_router();

    let request = Request::builder().uri("/test.json").body(axum::body::Body::empty()).unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response.headers().get("content-type").unwrap();
    assert!(content_type.to_str().unwrap().contains("application/json"));
}

#[tokio::test]
async fn test_serve_subdirectory_file() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = setup_test_files(&temp_dir).await;

    let handler = StaticHandler::new(temp_path).unwrap();
    let app = handler.create_router();

    let request = Request::builder()
        .uri("/assets/logo.svg")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response.headers().get("content-type").unwrap();
    assert!(content_type.to_str().unwrap().contains("image/svg+xml"));
}

#[tokio::test]
async fn test_file_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = setup_test_files(&temp_dir).await;

    let handler = StaticHandler::new(temp_path).unwrap();
    let app = handler.create_router();

    let request = Request::builder()
        .uri("/nonexistent.txt")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_spa_fallback_to_index() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = setup_test_files(&temp_dir).await;

    let handler = StaticHandler::new(temp_path).unwrap();
    let app = handler.create_router();

    // Request a non-existent file that should fallback to index.html for SPA support
    let request = Request::builder().uri("/app/dashboard").body(axum::body::Body::empty()).unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let content = String::from_utf8(body.to_vec()).unwrap();
    assert!(content.contains("Home Page")); // Should serve index.html content
}

#[tokio::test]
async fn test_directory_traversal_protection() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = setup_test_files(&temp_dir).await;

    let handler = StaticHandler::new(temp_path).unwrap();
    let app = handler.create_router();

    // Try to access files outside the base directory
    let request = Request::builder()
        .uri("/../../../etc/passwd")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_cache_headers() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = setup_test_files(&temp_dir).await;

    let handler = StaticHandler::new(temp_path).unwrap();
    let app = handler.create_router();

    let request = Request::builder().uri("/test.css").body(axum::body::Body::empty()).unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Check that cache control headers are set
    let cache_control = response.headers().get("cache-control").unwrap();
    assert!(cache_control.to_str().unwrap().contains("max-age=3600"));
}
