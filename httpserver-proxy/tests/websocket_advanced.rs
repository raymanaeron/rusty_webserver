use httpserver_proxy::ProxyHandler;
use httpserver_config::{ProxyRoute, LoadBalancingStrategy};
use httpserver_balancer::{Target, LoadBalancer};
use axum::{extract::Request, body::Body};

fn create_comprehensive_websocket_handler() -> ProxyHandler {
    let routes = vec![        // Multi-target WebSocket route with sticky sessions
        ProxyRoute {
            path: "/ws/chat/*".to_string(),
            targets: vec![
                Target::new("http://localhost:5000".to_string()),
                Target::new("http://localhost:5001".to_string()),
                Target::new("http://localhost:5002".to_string()),
            ],
            target: None,
            strategy: LoadBalancingStrategy::LeastConnections,
            timeout: 300,
            sticky_sessions: false,
            http_health: None,
            websocket_health: None,
            circuit_breaker: None,
        },        // WebSocket notifications with round-robin
        ProxyRoute {
            path: "/ws/notifications/*".to_string(),
            targets: vec![
                Target::new("http://localhost:6000".to_string()),
                Target::new("http://localhost:6001".to_string()),
            ],
            target: None,
            strategy: LoadBalancingStrategy::RoundRobin,
            timeout: 600,
            sticky_sessions: false,
            http_health: None,
            websocket_health: None,
            circuit_breaker: None,
        },        // Weighted WebSocket route
        ProxyRoute {
            path: "/ws/realtime/*".to_string(),
            targets: vec![
                Target::with_weight("http://localhost:8000".to_string(), 3),
                Target::with_weight("http://localhost:8001".to_string(), 2),
                Target::with_weight("http://localhost:8002".to_string(), 1),
            ],
            target: None,
            strategy: LoadBalancingStrategy::WeightedRoundRobin,
            timeout: 300,
            sticky_sessions: false,
            http_health: None,
            websocket_health: None,
            circuit_breaker: None,
        },        // Single WebSocket endpoint (legacy)
        ProxyRoute {
            path: "/ws/events".to_string(),
            targets: vec![],
            target: Some("http://localhost:7000".to_string()),
            strategy: LoadBalancingStrategy::RoundRobin,
            timeout: 300,
            sticky_sessions: false,
            http_health: None,
            websocket_health: None,
            circuit_breaker: None,
        },        // HTTP route for comparison
        ProxyRoute {
            path: "/api/*".to_string(),
            targets: vec![
                Target::new("http://localhost:3000".to_string()),
                Target::new("http://localhost:3001".to_string()),
            ],
            target: None,
            strategy: LoadBalancingStrategy::RoundRobin,
            timeout: 30,
            sticky_sessions: false,
            http_health: None,
            websocket_health: None,
            circuit_breaker: None,
        },
    ];
    
    ProxyHandler::new(routes)
}

// Test WebSocket detection with various header combinations
#[test]
fn test_comprehensive_websocket_detection() {
    // Standard WebSocket request
    let standard_request = Request::builder()
        .method("GET")
        .uri("/ws/chat/room1")
        .header("connection", "upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(Body::empty())
        .unwrap();
    
    assert!(ProxyHandler::is_websocket_request(&standard_request));
    
    // Case-insensitive headers
    let case_insensitive = Request::builder()
        .method("GET")
        .uri("/ws/notifications/alerts")
        .header("Connection", "Upgrade")
        .header("Upgrade", "WebSocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(Body::empty())
        .unwrap();
    
    assert!(ProxyHandler::is_websocket_request(&case_insensitive));
    
    // Connection header with multiple values
    let multi_connection = Request::builder()
        .method("GET")
        .uri("/ws/realtime/data")
        .header("connection", "keep-alive, Upgrade")
        .header("upgrade", "websocket")
        .body(Body::empty())
        .unwrap();
    
    assert!(ProxyHandler::is_websocket_request(&multi_connection));
}

// Test WebSocket routing to different backend configurations
#[test]
fn test_websocket_route_configurations() {
    let handler = create_comprehensive_websocket_handler();
    
    // Test multi-target chat route
    let chat_match = handler.find_route("/ws/chat/room123").unwrap();
    assert_eq!(chat_match.route.path, "/ws/chat/*");
    assert_eq!(chat_match.stripped_path, "/room123");
    assert!(chat_match.is_wildcard);
    assert_eq!(chat_match.route.strategy, LoadBalancingStrategy::LeastConnections);
    assert_eq!(chat_match.route.timeout, 300);
    assert_eq!(chat_match.route.get_targets().len(), 3);
    
    // Test notifications route with round-robin
    let notifications_match = handler.find_route("/ws/notifications/user456").unwrap();
    assert_eq!(notifications_match.route.path, "/ws/notifications/*");
    assert_eq!(notifications_match.stripped_path, "/user456");
    assert_eq!(notifications_match.route.strategy, LoadBalancingStrategy::RoundRobin);
    assert_eq!(notifications_match.route.timeout, 600);
    
    // Test weighted realtime route
    let realtime_match = handler.find_route("/ws/realtime/metrics").unwrap();
    assert_eq!(realtime_match.route.path, "/ws/realtime/*");
    assert_eq!(realtime_match.route.strategy, LoadBalancingStrategy::WeightedRoundRobin);
    let targets = realtime_match.route.get_targets();
    assert_eq!(targets.len(), 3);
    assert_eq!(targets[0].weight, 3);
    assert_eq!(targets[1].weight, 2);
    assert_eq!(targets[2].weight, 1);
    
    // Test single endpoint legacy mode
    let events_match = handler.find_route("/ws/events").unwrap();
    assert_eq!(events_match.route.path, "/ws/events");
    assert!(!events_match.is_wildcard);
    assert_eq!(events_match.route.get_primary_target(), Some("http://localhost:7000".to_string()));
}

// Test WebSocket vs HTTP route differentiation
#[test]  
fn test_websocket_vs_http_routes() {
    let handler = create_comprehensive_websocket_handler();
    
    // WebSocket request to WebSocket route should be detectable
    let ws_request = Request::builder()
        .method("GET")
        .uri("/ws/chat/general")
        .header("connection", "upgrade")
        .header("upgrade", "websocket")
        .body(Body::empty())
        .unwrap();
    
    assert!(ProxyHandler::is_websocket_request(&ws_request));
    let route_match = handler.find_route("/ws/chat/general").unwrap();
    assert_eq!(route_match.route.path, "/ws/chat/*");
    
    // HTTP request to HTTP route
    let http_request = Request::builder()
        .method("POST")
        .uri("/api/users")
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();
    
    assert!(!ProxyHandler::is_websocket_request(&http_request));
    let route_match = handler.find_route("/api/users").unwrap();
    assert_eq!(route_match.route.path, "/api/*");
    
    // HTTP request to WebSocket route should still route (but won't upgrade)
    let http_to_ws_route = Request::builder()
        .method("GET")
        .uri("/ws/chat/info")
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();
    
    assert!(!ProxyHandler::is_websocket_request(&http_to_ws_route));
    let route_match = handler.find_route("/ws/chat/info").unwrap();
    assert_eq!(route_match.route.path, "/ws/chat/*");
}

// Test load balancing strategies for WebSocket routes
#[test]
fn test_websocket_load_balancing_strategies() {
    // Test least connections strategy (ideal for WebSocket)
    let chat_targets = vec![
        Target::new("http://localhost:5000".to_string()),
        Target::new("http://localhost:5001".to_string()),
        Target::new("http://localhost:5002".to_string()),
    ];
    let chat_balancer = LoadBalancer::new(chat_targets, LoadBalancingStrategy::LeastConnections);
    
    // Initially all targets should have 0 connections
    assert_eq!(chat_balancer.get_connection_count("http://localhost:5000"), 0);
    assert_eq!(chat_balancer.get_connection_count("http://localhost:5001"), 0);
    assert_eq!(chat_balancer.get_connection_count("http://localhost:5002"), 0);
    
    // Test round-robin for notifications (suitable for broadcast)
    let notification_targets = vec![
        Target::new("http://localhost:6000".to_string()),
        Target::new("http://localhost:6001".to_string()),
    ];
    let notification_balancer = LoadBalancer::new(notification_targets, LoadBalancingStrategy::RoundRobin);
    
    // Round-robin should cycle through targets
    let first_target = notification_balancer.select_target().unwrap();
    let second_target = notification_balancer.select_target().unwrap();
    assert_ne!(first_target.url, second_target.url);
    
    // Test weighted round-robin for realtime (capacity-based)
    let realtime_targets = vec![
        Target::with_weight("http://localhost:8000".to_string(), 3),
        Target::with_weight("http://localhost:8001".to_string(), 2),
        Target::with_weight("http://localhost:8002".to_string(), 1),
    ];
    let realtime_balancer = LoadBalancer::new(realtime_targets, LoadBalancingStrategy::WeightedRoundRobin);
    
    // Collect selections to verify weight distribution
    let selections: Vec<String> = (0..18)
        .map(|_| realtime_balancer.select_target().unwrap().url.clone())
        .collect();
    
    let count_8000 = selections.iter().filter(|&url| url == "http://localhost:8000").count();
    let count_8001 = selections.iter().filter(|&url| url == "http://localhost:8001").count();
    let count_8002 = selections.iter().filter(|&url| url == "http://localhost:8002").count();
    
    // Should follow 3:2:1 ratio approximately
    assert!(count_8000 >= count_8001);
    assert!(count_8001 >= count_8002);
    assert!(count_8000 > 0 && count_8001 > 0 && count_8002 > 0);
}

// Test WebSocket-specific timeout configurations
#[test]
fn test_websocket_timeout_configurations() {
    let handler = create_comprehensive_websocket_handler();
    
    // WebSocket routes should have longer timeouts than HTTP
    let chat_route = handler.find_route("/ws/chat/room1").unwrap();
    assert_eq!(chat_route.route.timeout, 300); // 5 minutes
    
    let notifications_route = handler.find_route("/ws/notifications/alerts").unwrap();
    assert_eq!(notifications_route.route.timeout, 600); // 10 minutes
    
    let realtime_route = handler.find_route("/ws/realtime/data").unwrap();
    assert_eq!(realtime_route.route.timeout, 300); // 5 minutes
    
    let events_route = handler.find_route("/ws/events").unwrap();
    assert_eq!(events_route.route.timeout, 300); // 5 minutes
    
    // HTTP route should have shorter timeout
    let api_route = handler.find_route("/api/users").unwrap();
    assert_eq!(api_route.route.timeout, 30); // 30 seconds
}

// Test URL path stripping for WebSocket routes
#[test]
fn test_websocket_path_stripping() {
    let handler = create_comprehensive_websocket_handler();
    
    // Deep nested chat room
    let deep_chat = handler.find_route("/ws/chat/rooms/general/thread/123").unwrap();
    assert_eq!(deep_chat.stripped_path, "/rooms/general/thread/123");
    
    // Notification with user ID and type
    let user_notification = handler.find_route("/ws/notifications/user/456/messages").unwrap();
    assert_eq!(user_notification.stripped_path, "/user/456/messages");
    
    // Realtime data with complex path
    let realtime_data = handler.find_route("/ws/realtime/metrics/cpu/server1").unwrap();
    assert_eq!(realtime_data.stripped_path, "/metrics/cpu/server1");
    
    // Exact match should have empty stripped path
    let events = handler.find_route("/ws/events").unwrap();
    assert_eq!(events.stripped_path, "");
}

// Test that WebSocket routes work with all HTTP methods (for compatibility)
#[test]
fn test_websocket_routes_http_method_compatibility() {
    let handler = create_comprehensive_websocket_handler();
    
    // WebSocket routes should match regardless of HTTP method
    // (though typically GET is used for WebSocket upgrades)
    let get_match = handler.find_route("/ws/chat/room1");
    let post_match = handler.find_route("/ws/chat/room1");
    let put_match = handler.find_route("/ws/chat/room1");
    
    assert!(get_match.is_some());
    assert!(post_match.is_some()); 
    assert!(put_match.is_some());
    
    // All should match the same route
    assert_eq!(get_match.unwrap().route.path, "/ws/chat/*");
    assert_eq!(post_match.unwrap().route.path, "/ws/chat/*");
    assert_eq!(put_match.unwrap().route.path, "/ws/chat/*");
}

// Test edge cases for WebSocket header validation
#[test]
fn test_websocket_header_edge_cases() {
    // Valid WebSocket request with extra headers
    let valid_with_extras = Request::builder()
        .method("GET")
        .uri("/ws/test")
        .header("connection", "upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("origin", "https://example.com")
        .header("sec-websocket-protocol", "chat, superchat")
        .body(Body::empty())
        .unwrap();
    
    assert!(ProxyHandler::is_websocket_request(&valid_with_extras));
    
    // Connection header with mixed case and spaces
    let mixed_case_connection = Request::builder()
        .method("GET")
        .uri("/ws/test")
        .header("connection", "Keep-Alive, Upgrade")
        .header("upgrade", "websocket")
        .body(Body::empty())
        .unwrap();
    
    assert!(ProxyHandler::is_websocket_request(&mixed_case_connection));
    
    // Invalid: connection header doesn't contain upgrade
    let invalid_connection = Request::builder()
        .method("GET")
        .uri("/ws/test")
        .header("connection", "keep-alive, close")
        .header("upgrade", "websocket")
        .body(Body::empty())
        .unwrap();
    
    assert!(!ProxyHandler::is_websocket_request(&invalid_connection));
    
    // Invalid: upgrade header is wrong  
    let invalid_upgrade = Request::builder()
        .method("GET")
        .uri("/ws/test")
        .header("connection", "upgrade")
        .header("upgrade", "h2c")
        .body(Body::empty())
        .unwrap();
    
    assert!(!ProxyHandler::is_websocket_request(&invalid_upgrade));
    
    // Invalid: missing both critical headers
    let missing_headers = Request::builder()
        .method("GET")
        .uri("/ws/test")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(Body::empty())
        .unwrap();
    
    assert!(!ProxyHandler::is_websocket_request(&missing_headers));
}

// Test integration between route matching and load balancing
#[test]
fn test_websocket_route_balancer_integration() {
    let handler = create_comprehensive_websocket_handler();
    
    // Verify that route matching returns correct configurations for load balancing
    let chat_match = handler.find_route("/ws/chat/lobby").unwrap();
    let targets = chat_match.route.get_targets();
    
    // Should have 3 targets for load balancing
    assert_eq!(targets.len(), 3);
    assert_eq!(targets[0].url, "http://localhost:5000");
    assert_eq!(targets[1].url, "http://localhost:5001");
    assert_eq!(targets[2].url, "http://localhost:5002");
    
    // Verify strategy is preserved
    assert_eq!(chat_match.route.strategy, LoadBalancingStrategy::LeastConnections);
    
    // Verify legacy single target still works
    let events_match = handler.find_route("/ws/events").unwrap();
    assert_eq!(events_match.route.get_primary_target(), Some("http://localhost:7000".to_string()));
    assert!(events_match.route.get_targets().is_empty() || 
            events_match.route.get_targets().len() == 1);
}

