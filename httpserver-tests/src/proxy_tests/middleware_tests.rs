use httpserver_proxy::{ ProxyHandler, MiddlewareProcessor, MiddlewareError };
use httpserver_config::{
    ProxyRoute,
    LoadBalancingStrategy,
    MiddlewareConfig,
    HeaderMiddlewareConfig,
    RateLimitConfig,
    TransformConfig,
    RequestTransformConfig,
    ResponseTransformConfig,
    TextReplacement,
    AuthMiddlewareConfig,
    ApiKeyConfig,
    CompressionConfig,
};
use httpserver_balancer::Target;
use axum::{ extract::Request, body::Body };
use serde_json::json;
use std::{ collections::HashMap, net::SocketAddr, str::FromStr };

fn create_test_proxy_handler_with_middleware() -> ProxyHandler {
    let mut request_headers = HashMap::new();
    request_headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());
    request_headers.insert("X-Request-ID".to_string(), "test-123".to_string());

    let mut response_headers = HashMap::new();
    response_headers.insert("X-Powered-By".to_string(), "RustyWebServer".to_string());
    response_headers.insert("X-Response-Time".to_string(), "fast".to_string());

    let middleware_config = MiddlewareConfig {
        headers: Some(HeaderMiddlewareConfig {
            request_headers,
            response_headers,
            remove_request_headers: vec!["X-Remove-Me".to_string()],
            remove_response_headers: vec!["Server".to_string()],
            override_host: Some("backend.example.com".to_string()),
        }),
        rate_limit: Some(RateLimitConfig {
            requests_per_minute: 60,
            window_seconds: 60,
            max_concurrent: 5,
            limit_by_header: None,
            rate_limit_message: "Rate limit exceeded!".to_string(),
        }),
        transform: Some(TransformConfig {
            request: Some(RequestTransformConfig {
                replace_text: vec![TextReplacement {
                    find: "oldvalue".to_string(),
                    replace: "newvalue".to_string(),
                    regex_enabled: false,
                }],
                add_json_fields: {
                    let mut fields = HashMap::new();
                    fields.insert("middleware_processed".to_string(), json!(true));
                    fields
                },
                remove_json_fields: vec!["sensitive_data".to_string()],
            }),
            response: Some(ResponseTransformConfig {
                replace_text: vec![TextReplacement {
                    find: "internal".to_string(),
                    replace: "external".to_string(),
                    regex_enabled: false,
                }],
                add_json_fields: {
                    let mut fields = HashMap::new();
                    fields.insert("processed_by".to_string(), json!("gateway"));
                    fields
                },
                remove_json_fields: vec!["internal_id".to_string()],
            }),
        }),
        auth: Some(AuthMiddlewareConfig {
            bearer_token: Some("test-token-123".to_string()),
            basic_auth: None,
            custom_auth_header: Some(("X-API-Key".to_string(), "secret-key".to_string())),
            api_key: Some(ApiKeyConfig {
                header_name: "Authorization-Key".to_string(),
                key_value: "api-key-456".to_string(),
            }),
        }),
        compression: Some(CompressionConfig {
            gzip: true,
            brotli: false,
            threshold_bytes: 100,
            level: 6,
        }),
    };    let routes = vec![ProxyRoute {
        path: "/api/*".to_string(),
        targets: vec![Target::new("http://localhost:3000".to_string())],
        target: None,
        strategy: LoadBalancingStrategy::RoundRobin,
        timeout: 30,
        sticky_sessions: false,
        http_health: None,
        websocket_health: None,
        circuit_breaker: None,
        middleware: Some(middleware_config),
        ssl: None,
    }];

    ProxyHandler::new(routes)
}

fn create_middleware_processor() -> MiddlewareProcessor {
    MiddlewareProcessor::new()
}

#[test]
fn test_middleware_processor_creation() {
    let processor = create_middleware_processor();
    // Basic creation test - should not panic
    drop(processor);
}

#[test]
fn test_proxy_handler_with_middleware_creation() {
    let handler = create_test_proxy_handler_with_middleware();
    assert!(handler.has_routes());
    assert_eq!(handler.routes().len(), 1);

    // Verify middleware is configured
    let route = &handler.routes()[0];
    assert!(route.middleware.is_some());

    let middleware = route.middleware.as_ref().unwrap();
    assert!(middleware.headers.is_some());
    assert!(middleware.rate_limit.is_some());
    assert!(middleware.transform.is_some());
    assert!(middleware.auth.is_some());
    assert!(middleware.compression.is_some());
}

#[tokio::test]
async fn test_header_injection_middleware() {
    let processor = create_middleware_processor();
    let client_ip = SocketAddr::from_str("127.0.0.1:12345").unwrap();

    let mut request_headers = HashMap::new();
    request_headers.insert("X-Test".to_string(), "value".to_string());

    let middleware_config = MiddlewareConfig {
        headers: Some(HeaderMiddlewareConfig {
            request_headers,
            response_headers: HashMap::new(),
            remove_request_headers: vec![],
            remove_response_headers: vec![],
            override_host: Some("example.com".to_string()),
        }),
        rate_limit: None,
        transform: None,
        auth: None,
        compression: None,
    };

    let req = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();

    let result = processor.process_request(req, &client_ip, &middleware_config).await;
    assert!(result.is_ok());

    let processed_req = result.unwrap();
    let headers = processed_req.headers();

    assert_eq!(headers.get("X-Test").unwrap(), "value");
    assert_eq!(headers.get("host").unwrap(), "example.com");
}

#[tokio::test]
async fn test_authentication_middleware() {
    let processor = create_middleware_processor();
    let client_ip = SocketAddr::from_str("127.0.0.1:12345").unwrap();

    let middleware_config = MiddlewareConfig {
        headers: None,
        rate_limit: None,
        transform: None,
        auth: Some(AuthMiddlewareConfig {
            bearer_token: Some("test-token".to_string()),
            basic_auth: None,
            custom_auth_header: Some(("X-API-Key".to_string(), "secret".to_string())),
            api_key: None,
        }),
        compression: None,
    };

    let req = Request::builder().method("POST").uri("/api/test").body(Body::empty()).unwrap();

    let result = processor.process_request(req, &client_ip, &middleware_config).await;
    assert!(result.is_ok());

    let processed_req = result.unwrap();
    let headers = processed_req.headers();

    assert_eq!(headers.get("authorization").unwrap(), "Bearer test-token");
    assert_eq!(headers.get("X-API-Key").unwrap(), "secret");
}

#[tokio::test]
async fn test_rate_limiting_middleware() {
    let processor = create_middleware_processor();
    let client_ip = SocketAddr::from_str("127.0.0.1:12345").unwrap();
    let middleware_config = MiddlewareConfig {
        headers: None,
        rate_limit: Some(RateLimitConfig {
            requests_per_minute: 2, // Very low limit for testing
            window_seconds: 60,
            max_concurrent: 10, // Higher limit to allow multiple requests
            limit_by_header: None,
            rate_limit_message: "Too many requests".to_string(),
        }),
        transform: None,
        auth: None,
        compression: None,
    };

    let req1 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();

    let req2 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();

    let req3 = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();

    // First request should succeed
    let result1 = processor.process_request(req1, &client_ip, &middleware_config).await;
    assert!(result1.is_ok());

    // Second request should succeed
    let result2 = processor.process_request(req2, &client_ip, &middleware_config).await;
    assert!(result2.is_ok());

    // Third request should be rate limited
    let result3 = processor.process_request(req3, &client_ip, &middleware_config).await;
    assert!(result3.is_err());

    if let Err(MiddlewareError::RateLimitExceeded(msg)) = result3 {
        assert_eq!(msg, "Too many requests");
    } else {
        panic!("Expected rate limit error");
    }
}

#[tokio::test]
async fn test_json_transformation_middleware() {
    let processor = create_middleware_processor();
    let client_ip = SocketAddr::from_str("127.0.0.1:12345").unwrap();

    let mut add_fields = HashMap::new();
    add_fields.insert("added_field".to_string(), json!("added_value"));

    let middleware_config = MiddlewareConfig {
        headers: None,
        rate_limit: None,
        transform: Some(TransformConfig {
            request: Some(RequestTransformConfig {
                replace_text: vec![],
                add_json_fields: add_fields,
                remove_json_fields: vec!["remove_me".to_string()],
            }),
            response: None,
        }),
        auth: None,
        compression: None,
    };

    let json_body =
        json!({
        "existing_field": "value",
        "remove_me": "should_be_removed"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/test")
        .header("content-type", "application/json")
        .body(Body::from(json_body.to_string()))
        .unwrap();

    let result = processor.process_request(req, &client_ip, &middleware_config).await;
    assert!(result.is_ok());

    let processed_req = result.unwrap();
    let body_bytes = axum::body::to_bytes(processed_req.into_body(), usize::MAX).await.unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    let parsed_json: serde_json::Value = serde_json::from_str(&body_str).unwrap();

    // Check that field was added
    assert_eq!(parsed_json["added_field"], "added_value");
    assert_eq!(parsed_json["existing_field"], "value");

    // Check that field was removed
    assert!(parsed_json.get("remove_me").is_none());
}

#[tokio::test]
async fn test_text_replacement_middleware() {
    let processor = create_middleware_processor();
    let client_ip = SocketAddr::from_str("127.0.0.1:12345").unwrap();

    let middleware_config = MiddlewareConfig {
        headers: None,
        rate_limit: None,
        transform: Some(TransformConfig {
            request: Some(RequestTransformConfig {
                replace_text: vec![TextReplacement {
                    find: "oldtext".to_string(),
                    replace: "newtext".to_string(),
                    regex_enabled: false,
                }],
                add_json_fields: HashMap::new(),
                remove_json_fields: vec![],
            }),
            response: None,
        }),
        auth: None,
        compression: None,
    };

    let req = Request::builder()
        .method("POST")
        .uri("/api/test")
        .body(Body::from("This contains oldtext that should be replaced"))
        .unwrap();

    let result = processor.process_request(req, &client_ip, &middleware_config).await;
    assert!(result.is_ok());

    let processed_req = result.unwrap();
    let body_bytes = axum::body::to_bytes(processed_req.into_body(), usize::MAX).await.unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    assert_eq!(body_str, "This contains newtext that should be replaced");
}

#[tokio::test]
async fn test_compression_middleware() {
    let processor = create_middleware_processor();

    let middleware_config = MiddlewareConfig {
        headers: None,
        rate_limit: None,
        transform: None,
        auth: None,
        compression: Some(CompressionConfig {
            gzip: true,
            brotli: false,
            threshold_bytes: 10, // Very low threshold for testing
            level: 6,
        }),
    };

    let large_body =
        "This is a large response body that should be compressed because it exceeds the threshold size of 10 bytes.";

    let response = axum::response::Response
        ::builder()
        .status(200)
        .body(Body::from(large_body))
        .unwrap();

    let result = processor.process_response(response, &middleware_config).await;
    assert!(result.is_ok());

    let processed_response = result.unwrap();
    let headers = processed_response.headers();

    // Check that compression headers were added
    assert_eq!(headers.get("content-encoding").unwrap(), "gzip");
    assert!(headers.contains_key("content-length"));

    // Verify the body is actually compressed (should be smaller)
    let compressed_body = axum::body
        ::to_bytes(processed_response.into_body(), usize::MAX).await
        .unwrap();
    assert!(compressed_body.len() < large_body.len());
}

#[test]
fn test_middleware_configuration_defaults() {
    // Test that middleware configurations have sensible defaults
    let rate_config = RateLimitConfig {
        requests_per_minute: 100,
        window_seconds: 60,
        max_concurrent: 10,
        limit_by_header: None,
        rate_limit_message: "Rate limit exceeded. Please try again later.".to_string(),
    };

    assert_eq!(rate_config.requests_per_minute, 100);
    assert_eq!(rate_config.window_seconds, 60);
    assert_eq!(rate_config.max_concurrent, 10);

    let compression_config = CompressionConfig {
        gzip: true,
        brotli: false,
        threshold_bytes: 1024,
        level: 6,
    };

    assert!(compression_config.gzip);
    assert!(!compression_config.brotli);
    assert_eq!(compression_config.threshold_bytes, 1024);
    assert_eq!(compression_config.level, 6);
}

#[test]
fn test_middleware_error_display() {
    let rate_limit_error = MiddlewareError::RateLimitExceeded("Too many requests".to_string());
    assert_eq!(rate_limit_error.to_string(), "Rate limit exceeded: Too many requests");

    let header_error = MiddlewareError::HeaderError("Invalid header".to_string());
    assert_eq!(header_error.to_string(), "Header error: Invalid header");

    let transform_error = MiddlewareError::TransformError("JSON parse failed".to_string());
    assert_eq!(transform_error.to_string(), "Transform error: JSON parse failed");

    let auth_error = MiddlewareError::AuthError("Invalid token".to_string());
    assert_eq!(auth_error.to_string(), "Authentication error: Invalid token");

    let compression_error = MiddlewareError::CompressionError("Gzip failed".to_string());
    assert_eq!(compression_error.to_string(), "Compression error: Gzip failed");
}

#[tokio::test]
async fn test_header_removal_middleware() {
    let processor = create_middleware_processor();
    let client_ip = SocketAddr::from_str("127.0.0.1:12345").unwrap();

    let middleware_config = MiddlewareConfig {
        headers: Some(HeaderMiddlewareConfig {
            request_headers: HashMap::new(),
            response_headers: HashMap::new(),
            remove_request_headers: vec!["X-Remove-Me".to_string()],
            remove_response_headers: vec!["Server".to_string()],
            override_host: None,
        }),
        rate_limit: None,
        transform: None,
        auth: None,
        compression: None,
    };

    let req = Request::builder()
        .method("GET")
        .uri("/test")
        .header("X-Remove-Me", "should-be-removed")
        .header("X-Keep-Me", "should-be-kept")
        .body(Body::empty())
        .unwrap();

    let result = processor.process_request(req, &client_ip, &middleware_config).await;
    assert!(result.is_ok());

    let processed_req = result.unwrap();
    let headers = processed_req.headers();

    assert!(headers.get("X-Remove-Me").is_none());
    assert_eq!(headers.get("X-Keep-Me").unwrap(), "should-be-kept");
}

#[tokio::test]
async fn test_basic_auth_middleware() {
    let processor = create_middleware_processor();
    let client_ip = SocketAddr::from_str("127.0.0.1:12345").unwrap();

    let middleware_config = MiddlewareConfig {
        headers: None,
        rate_limit: None,
        transform: None,
        auth: Some(AuthMiddlewareConfig {
            bearer_token: None,
            basic_auth: Some("username:password".to_string()),
            custom_auth_header: None,
            api_key: None,
        }),
        compression: None,
    };

    let req = Request::builder().method("GET").uri("/test").body(Body::empty()).unwrap();

    let result = processor.process_request(req, &client_ip, &middleware_config).await;
    assert!(result.is_ok());

    let processed_req = result.unwrap();
    let headers = processed_req.headers();

    let auth_header = headers.get("authorization").unwrap().to_str().unwrap();
    assert!(auth_header.starts_with("Basic "));

    // Verify the base64 encoding is correct
    use base64::{ Engine as _, engine::general_purpose };
    let expected = format!("Basic {}", general_purpose::STANDARD.encode("username:password"));
    assert_eq!(auth_header, expected);
}
