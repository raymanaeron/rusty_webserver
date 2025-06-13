// Health endpoint tests for httpserver-config
use httpserver_config::{config_health, create_config_health_router};
use axum::http::StatusCode;
use tower::ServiceExt;
use serde_json::Value;

#[tokio::test]
async fn test_config_health_endpoint() {
    let response = config_health().await;
    let json_value: Value = response.0;
    
    assert_eq!(json_value["status"], "healthy");
    assert_eq!(json_value["service"], "httpserver-config");
    assert_eq!(json_value["message"], "Configuration parsing service operational");
}

#[tokio::test]
async fn test_config_health_router() {
    let app = create_config_health_router();
    
    // Test /config/health endpoint
    let request = axum::http::Request::builder()
        .uri("/config/health")
        .body(axum::body::Body::empty())
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    // Test /config/status endpoint
    let request = axum::http::Request::builder()
        .uri("/config/status")
        .body(axum::body::Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn test_config_health_router_creation() {
    let _router = create_config_health_router();
    
    // Verify router is created successfully (just test compilation)
}
