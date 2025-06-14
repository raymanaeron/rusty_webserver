use httpserver_balancer::{ balancer_health, create_balancer_health_router };
use axum::Router;
use serde_json::Value;
use tokio::time::Duration;

#[tokio::test]
async fn test_balancer_health_endpoint() {
    let response = balancer_health().await;
    let health_data: Value = response.0;
    assert_eq!(health_data["status"], "healthy");
    assert_eq!(health_data["service"], "httpserver-balancer");
    assert_eq!(health_data["message"], "Load balancing service operational");
}

#[tokio::test]
async fn test_balancer_health_router_creation() {
    let router = create_balancer_health_router();

    // Test that we can create a service from the router
    let app = Router::new().merge(router);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.unwrap();
    });

    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test balancer health endpoint
    let client = reqwest::Client::new();
    let response = client.get(format!("http://{}/balancer/health", addr)).send().await.unwrap();

    assert_eq!(response.status(), 200);
    let health_data: Value = response.json().await.unwrap();
    assert_eq!(health_data["status"], "healthy");
    assert_eq!(health_data["service"], "httpserver-balancer");
}

#[tokio::test]
async fn test_balancer_status_endpoint() {
    let router = create_balancer_health_router();
    let app = Router::new().merge(router);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let response = client.get(format!("http://{}/balancer/status", addr)).send().await.unwrap();

    assert_eq!(response.status(), 200);
    let health_data: Value = response.json().await.unwrap();
    assert_eq!(health_data["status"], "healthy");
}

#[tokio::test]
async fn test_health_endpoint_response_format() {
    let response = balancer_health().await;
    let health_data: Value = response.0;

    // Check that all expected fields are present
    assert!(health_data.get("status").is_some());
    assert!(health_data.get("service").is_some());
    assert!(health_data.get("message").is_some());

    // Check data types
    assert!(health_data["status"].is_string());
    assert!(health_data["service"].is_string());
    assert!(health_data["message"].is_string());
}

#[tokio::test]
async fn test_health_router_integration() {
    // Test that the health router can be merged with other routers
    let health_router = create_balancer_health_router();
    let _main_router = Router::new().merge(health_router);

    // This test verifies that the routers can be combined without conflicts
    assert!(true);
}
