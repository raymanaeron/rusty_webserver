use httpserver_static::{StaticHandler, static_health, create_static_health_router};
use axum::Router;
use std::path::PathBuf;
use tempfile::TempDir;
use serde_json::Value;

#[tokio::test]
async fn test_static_handler_creation_valid_dir() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    
    let handler = StaticHandler::new(temp_path.clone());
    assert!(handler.is_ok());
    
    let handler = handler.unwrap();
    assert_eq!(handler.base_dir.file_name(), temp_path.file_name());
}

#[tokio::test]
async fn test_static_handler_creation_invalid_dir() {
    let invalid_path = PathBuf::from("c:\\nonexistent\\directory\\that\\does\\not\\exist");
    
    let handler = StaticHandler::new(invalid_path);
    assert!(handler.is_err());
}

#[tokio::test]
async fn test_static_health_endpoint() {
    let response = static_health().await;
    let health_data: Value = response.0;
    
    assert_eq!(health_data["status"], "healthy");
    assert_eq!(health_data["service"], "httpserver-static");
    assert_eq!(health_data["message"], "Static file serving operational");
}

#[tokio::test]
async fn test_static_health_router_creation() {
    let router = create_static_health_router();
    
    // Test that we can create a service from the router
    let app = Router::new().merge(router);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.unwrap();
    });
    
    // Give the server a moment to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Test static health endpoint
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}/static/health", addr))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    let health_data: Value = response.json().await.unwrap();
    assert_eq!(health_data["status"], "healthy");
    assert_eq!(health_data["service"], "httpserver-static");
}

#[tokio::test]
async fn test_static_status_endpoint() {
    let router = create_static_health_router();
    let app = Router::new().merge(router);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.unwrap();
    });
    
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}/static/status", addr))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    let health_data: Value = response.json().await.unwrap();
    assert_eq!(health_data["status"], "healthy");
}

#[tokio::test]
async fn test_router_creation() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    
    let handler = StaticHandler::new(temp_path).unwrap();
    let _router = handler.create_router();
    
    // Just test that the router can be created without errors
    // The actual file serving functionality would require more complex setup
    assert!(true);
}
