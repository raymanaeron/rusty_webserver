use httpserver_proxy::ProxyHandler;
use httpserver_config::{ProxyRoute, LoadBalancingStrategy};
use httpserver_balancer::Target;
use axum::{extract::Request, body::Body};

fn create_test_proxy_handler() -> ProxyHandler {
    let routes = vec![
        ProxyRoute {
            path: "/ws/*".to_string(),
            targets: vec![
                Target::new("http://localhost:8001".to_string()),
                Target::new("http://localhost:8002".to_string()),
            ],
            target: None,
            strategy: LoadBalancingStrategy::RoundRobin,
            timeout: 30,
        },
        ProxyRoute {
            path: "/api/websocket".to_string(),
            targets: vec![],
            target: Some("http://localhost:9000".to_string()),
            strategy: LoadBalancingStrategy::RoundRobin,
            timeout: 30,
        },
    ];
    
    ProxyHandler::new(routes)
}

#[test]
fn test_websocket_detection() {
    let request = Request::builder()
        .method("GET")
        .uri("/ws/chat")
        .header("connection", "upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(Body::empty())
        .unwrap();
    
    assert!(ProxyHandler::is_websocket_request(&request));
}

#[test]
fn test_non_websocket_detection() {
    let request = Request::builder()
        .method("GET")
        .uri("/api/data")
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();
    
    assert!(!ProxyHandler::is_websocket_request(&request));
}

#[test]
fn test_websocket_route_matching() {
    let handler = create_test_proxy_handler();
    
    // Test wildcard WebSocket route matching
    let route_match = handler.find_route("/ws/chat/room1");
    assert!(route_match.is_some());
    
    let route_match = route_match.unwrap();
    assert_eq!(route_match.route.path, "/ws/*");
    assert_eq!(route_match.stripped_path, "/chat/room1");
    assert!(route_match.is_wildcard);
    
    // Test exact WebSocket route matching
    let exact_match = handler.find_route("/api/websocket");
    assert!(exact_match.is_some());
    
    let exact_match = exact_match.unwrap();
    assert_eq!(exact_match.route.path, "/api/websocket");
    assert_eq!(exact_match.stripped_path, "");
    assert!(!exact_match.is_wildcard);
}

#[test]
fn test_websocket_load_balancing() {
    let handler = create_test_proxy_handler();
    
    // Test that WebSocket routes can use load balancing
    let route_match = handler.find_route("/ws/notifications").unwrap();
    let targets = route_match.route.get_targets();
    assert_eq!(targets.len(), 2);
    assert_eq!(targets[0].url, "http://localhost:8001");
    assert_eq!(targets[1].url, "http://localhost:8002");
}

#[test]
fn test_websocket_headers_parsing() {
    // Test case-insensitive header parsing
    let request = Request::builder()
        .method("GET")
        .uri("/ws/test")
        .header("Connection", "Upgrade") // Capital C
        .header("Upgrade", "WebSocket") // Capital W and S
        .body(Body::empty())
        .unwrap();
    
    assert!(ProxyHandler::is_websocket_request(&request));
    
    // Test with connection header containing multiple values
    let request2 = Request::builder()
        .method("GET") 
        .uri("/ws/test")
        .header("connection", "keep-alive, upgrade")
        .header("upgrade", "websocket")
        .body(Body::empty())
        .unwrap();
    
    assert!(ProxyHandler::is_websocket_request(&request2));
}

#[test]
fn test_invalid_websocket_headers() {
    // Missing upgrade header
    let request1 = Request::builder()
        .method("GET")
        .uri("/ws/test")
        .header("connection", "upgrade")
        .body(Body::empty())
        .unwrap();
    
    assert!(!ProxyHandler::is_websocket_request(&request1));
    
    // Wrong upgrade value
    let request2 = Request::builder()
        .method("GET")
        .uri("/ws/test")
        .header("connection", "upgrade")
        .header("upgrade", "http2")
        .body(Body::empty())
        .unwrap();
    
    assert!(!ProxyHandler::is_websocket_request(&request2));
    
    // Missing connection header
    let request3 = Request::builder()
        .method("GET")
        .uri("/ws/test")
        .header("upgrade", "websocket")
        .body(Body::empty())
        .unwrap();
    
    assert!(!ProxyHandler::is_websocket_request(&request3));
}
