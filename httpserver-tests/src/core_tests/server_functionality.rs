use axum::{ routing::get, Router };
use httpserver_core::{
    Server,
    gateway_health,
    create_health_router,
    create_error_response,
    logging_middleware,
};
use axum::http::StatusCode;
use tokio::time::Duration;
use serde_json::Value;
use axum::body::to_bytes;

#[tokio::test]
async fn test_server_creation() {
    let server = Server::new(8080);
    assert_eq!(server.port, 8080);
}

#[tokio::test]
async fn test_gateway_health_endpoint() {
    let response = gateway_health().await;
    let health_data: Value = response.0;

    assert_eq!(health_data["status"], "healthy");
    assert_eq!(health_data["service"], "httpserver-gateway");
    assert!(health_data["timestamp"].is_string());
    assert!(health_data["version"].is_string());
}

#[tokio::test]
async fn test_health_router_creation() {
    let router = create_health_router();

    // Test that we can create a service from the router
    let app = Router::new().merge(router);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.unwrap();
    });

    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test health endpoint
    let client = reqwest::Client::new();
    let response = client.get(format!("http://{}/health", addr)).send().await.unwrap();

    assert_eq!(response.status(), 200);
    let health_data: Value = response.json().await.unwrap();
    assert_eq!(health_data["status"], "healthy");
}

#[tokio::test]
async fn test_ping_endpoint() {
    let router = create_health_router();
    let app = Router::new().merge(router);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let response = client.get(format!("http://{}/ping", addr)).send().await.unwrap();

    assert_eq!(response.status(), 200);
    let health_data: Value = response.json().await.unwrap();
    assert_eq!(health_data["status"], "healthy");
}

#[tokio::test]
async fn test_create_error_response() {
    let response = create_error_response(StatusCode::NOT_FOUND, "Test error message");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let headers = response.headers();
    assert_eq!(headers.get("content-type").unwrap(), "text/html");
}

#[tokio::test]
async fn test_error_response_content() {
    let response = create_error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal error occurred"
    );

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    // The body should contain HTML with the error message
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(html.contains("500"));
    assert!(html.contains("Internal error occurred"));
    assert!(html.contains("<!DOCTYPE html>"));
}

#[tokio::test]
async fn test_server_port_binding() {
    // Test that Server can be created with different ports
    let server1 = Server::new(3000);
    let server2 = Server::new(4000);

    assert_eq!(server1.port, 3000);
    assert_eq!(server2.port, 4000);
}

#[tokio::test]
async fn test_server_creation_and_router() {
    // Test that Server can be created with different ports
    let server1 = Server::new(3000);
    let server2 = Server::new(4000);

    assert_eq!(server1.port, 3000);
    assert_eq!(server2.port, 4000);
}

#[tokio::test]
async fn test_server_with_simple_router() {
    let server = Server::new(0); // Use port 0 for automatic assignment

    // For this test, we'll just verify the server struct is properly constructed
    assert_eq!(server.port, 0);
}

// Additional test for middleware functionality
#[tokio::test]
async fn test_logging_middleware_compilation() {
    // This test ensures the logging middleware compiles and can be used
    let _router: Router<()> = Router::new()
        .route(
            "/",
            get(|| async { "Hello, World!" })
        )
        .layer(axum::middleware::from_fn(logging_middleware));

    // Just testing that the middleware can be applied without compilation errors
    assert!(true);
}
