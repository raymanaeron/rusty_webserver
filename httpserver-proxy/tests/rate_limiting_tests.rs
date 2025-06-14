use httpserver_proxy::MiddlewareProcessor;
use httpserver_config::{MiddlewareConfig, RateLimitConfig};
use axum::{extract::Request, body::Body};
use std::{net::SocketAddr, str::FromStr, time::Duration};
use tokio::time::sleep;

fn create_rate_limit_config(requests_per_minute: u32, max_concurrent: u32) -> MiddlewareConfig {
    MiddlewareConfig {
        headers: None,
        rate_limit: Some(RateLimitConfig {
            requests_per_minute,
            window_seconds: 60,
            max_concurrent,
            limit_by_header: None,
            rate_limit_message: "Rate limit exceeded".to_string(),
        }),
        transform: None,
        auth: None,
        compression: None,
    }
}

#[tokio::test]
async fn test_rate_limiting_per_ip() {
    let processor = MiddlewareProcessor::new();
    let client_ip1 = SocketAddr::from_str("127.0.0.1:12345").unwrap();
    let client_ip2 = SocketAddr::from_str("192.168.1.100:54321").unwrap();
    
    let middleware_config = create_rate_limit_config(2, 5);
    
    // Client 1: Should allow 2 requests
    let req1 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    let req2 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    let req3 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    
    assert!(processor.process_request(req1, &client_ip1, &middleware_config).await.is_ok());
    assert!(processor.process_request(req2, &client_ip1, &middleware_config).await.is_ok());
    assert!(processor.process_request(req3, &client_ip1, &middleware_config).await.is_err());
    
    // Client 2: Should allow 2 requests (separate rate limit)
    let req4 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    let req5 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    
    assert!(processor.process_request(req4, &client_ip2, &middleware_config).await.is_ok());
    assert!(processor.process_request(req5, &client_ip2, &middleware_config).await.is_ok());
}

#[tokio::test]
async fn test_concurrent_connection_limiting() {
    let processor = MiddlewareProcessor::new();
    let client_ip = SocketAddr::from_str("127.0.0.1:12345").unwrap();
    
    let middleware_config = create_rate_limit_config(100, 2); // High request limit, low concurrent limit
    
    // Start 2 concurrent "requests" (don't finish them)
    let req1 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    let req2 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    let req3 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    
    // First two should succeed
    assert!(processor.process_request(req1, &client_ip, &middleware_config).await.is_ok());
    assert!(processor.process_request(req2, &client_ip, &middleware_config).await.is_ok());
    
    // Third should fail due to concurrent limit
    assert!(processor.process_request(req3, &client_ip, &middleware_config).await.is_err());
    
    // Finish one connection
    processor.finish_connection(&client_ip);
    
    // Now the next request should succeed
    let req4 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    assert!(processor.process_request(req4, &client_ip, &middleware_config).await.is_ok());
}

#[tokio::test]
async fn test_rate_limit_window_reset() {
    let processor = MiddlewareProcessor::new();
    let client_ip = SocketAddr::from_str("127.0.0.1:12345").unwrap();
    
    // Use a very short window for testing
    let middleware_config = MiddlewareConfig {
        headers: None,
        rate_limit: Some(RateLimitConfig {
            requests_per_minute: 1,
            window_seconds: 1, // 1 second window
            max_concurrent: 5,
            limit_by_header: None,
            rate_limit_message: "Rate limit exceeded".to_string(),
        }),
        transform: None,
        auth: None,
        compression: None,
    };
    
    // First request should succeed
    let req1 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    assert!(processor.process_request(req1, &client_ip, &middleware_config).await.is_ok());
    
    // Second request should fail (rate limited)
    let req2 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    assert!(processor.process_request(req2, &client_ip, &middleware_config).await.is_err());
    
    // Wait for window to reset
    sleep(Duration::from_millis(1100)).await;
    
    // Request should succeed again after window reset
    let req3 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    assert!(processor.process_request(req3, &client_ip, &middleware_config).await.is_ok());
}

#[test]
fn test_rate_limit_configuration_validation() {
    let config = RateLimitConfig {
        requests_per_minute: 100,
        window_seconds: 60,
        max_concurrent: 10,
        limit_by_header: Some("X-API-Key".to_string()),
        rate_limit_message: "Custom rate limit message".to_string(),
    };
    
    assert_eq!(config.requests_per_minute, 100);
    assert_eq!(config.window_seconds, 60);
    assert_eq!(config.max_concurrent, 10);
    assert_eq!(config.limit_by_header, Some("X-API-Key".to_string()));
    assert_eq!(config.rate_limit_message, "Custom rate limit message");
}

#[tokio::test]
async fn test_rate_limiting_with_custom_message() {
    let processor = MiddlewareProcessor::new();
    let client_ip = SocketAddr::from_str("127.0.0.1:12345").unwrap();
    
    let middleware_config = MiddlewareConfig {
        headers: None,
        rate_limit: Some(RateLimitConfig {
            requests_per_minute: 1,
            window_seconds: 60,
            max_concurrent: 5,
            limit_by_header: None,
            rate_limit_message: "Custom rate limit message!".to_string(),
        }),
        transform: None,
        auth: None,
        compression: None,
    };
    
    // First request should succeed
    let req1 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    assert!(processor.process_request(req1, &client_ip, &middleware_config).await.is_ok());
    
    // Second request should fail with custom message
    let req2 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    let result = processor.process_request(req2, &client_ip, &middleware_config).await;
    
    assert!(result.is_err());
    match result.unwrap_err() {
        httpserver_proxy::MiddlewareError::RateLimitExceeded(msg) => {
            assert_eq!(msg, "Custom rate limit message!");
        }
        _ => panic!("Expected RateLimitExceeded error"),
    }
}

#[tokio::test]
async fn test_connection_cleanup() {
    let processor = MiddlewareProcessor::new();
    let client_ip = SocketAddr::from_str("127.0.0.1:12345").unwrap();
    
    let middleware_config = create_rate_limit_config(100, 1); // Allow only 1 concurrent connection
    
    // Start a connection
    let req1 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    assert!(processor.process_request(req1, &client_ip, &middleware_config).await.is_ok());
    
    // Second connection should fail
    let req2 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    assert!(processor.process_request(req2, &client_ip, &middleware_config).await.is_err());
    
    // Finish the connection
    processor.finish_connection(&client_ip);
    
    // Now a new connection should succeed
    let req3 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    assert!(processor.process_request(req3, &client_ip, &middleware_config).await.is_ok());
    
    // Multiple finish_connection calls should be safe
    processor.finish_connection(&client_ip);
    processor.finish_connection(&client_ip);
    processor.finish_connection(&client_ip);
}

#[tokio::test]
async fn test_zero_rate_limit() {
    let processor = MiddlewareProcessor::new();
    let client_ip = SocketAddr::from_str("127.0.0.1:12345").unwrap();
    
    let middleware_config = create_rate_limit_config(0, 5); // No requests allowed
    
    // All requests should be rejected
    let req1 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    assert!(processor.process_request(req1, &client_ip, &middleware_config).await.is_err());
    
    let req2 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();
    assert!(processor.process_request(req2, &client_ip, &middleware_config).await.is_err());
}
