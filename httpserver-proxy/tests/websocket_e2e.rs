// End-to-end WebSocket test with real servers
use std::time::Duration;
use tokio::time::timeout;
use httpserver_proxy::{WebSocketHealthChecker};
use httpserver_config::WebSocketHealthConfig;

#[tokio::test]
async fn test_simple_websocket_health_check() {
    // Test WebSocket health checker with a non-existent server
    let health_config = WebSocketHealthConfig {
        interval: 5,
        timeout: 1, // Short timeout for fast test
        path: "".to_string(), // Empty path for direct connection
        ping_message: "ping".to_string(),
    };
    
    let health_checker = WebSocketHealthChecker::new(health_config);
    
    // Test health check against non-existent server (should fail quickly)
    let unhealthy_result = timeout(
        Duration::from_secs(2),
        health_checker.check_health("http://127.0.0.1:9999")
    ).await;
    
    assert!(unhealthy_result.is_ok(), "Health check should complete within timeout");
    assert!(!unhealthy_result.unwrap(), "Non-existent server should be unhealthy");
    
    println!("Simple WebSocket health check test completed");
}
