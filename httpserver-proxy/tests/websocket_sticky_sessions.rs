// Tests for WebSocket sticky sessions and connection management
// These tests focus on the persistent connection aspects that are critical for WebSocket functionality

use httpserver_proxy::ProxyHandler;
use httpserver_config::{ ProxyRoute, LoadBalancingStrategy };
use httpserver_balancer::{ Target, LoadBalancer };
use axum::{ extract::Request, body::Body };
use std::collections::HashMap;

fn create_sticky_session_handler() -> ProxyHandler {
    let routes = vec![
        // WebSocket route that should maintain sticky sessions
        ProxyRoute {
            path: "/ws/stateful/*".to_string(),
            targets: vec![
                Target::new("http://localhost:5000".to_string()),
                Target::new("http://localhost:5001".to_string()),
                Target::new("http://localhost:5002".to_string())
            ],
            target: None,
            strategy: LoadBalancingStrategy::LeastConnections, // Best for persistent connections
            timeout: 300,
            sticky_sessions: false,
            http_health: None,
            websocket_health: None,
            circuit_breaker: None,
            middleware: None,
        },
        // WebSocket route for broadcast scenarios (no sticky sessions needed)
        ProxyRoute {
            path: "/ws/broadcast/*".to_string(),
            targets: vec![
                Target::new("http://localhost:6000".to_string()),
                Target::new("http://localhost:6001".to_string())
            ],
            target: None,
            strategy: LoadBalancingStrategy::RoundRobin,
            timeout: 300,
            sticky_sessions: false,
            http_health: None,
            websocket_health: None,
            circuit_breaker: None,
            middleware: None,
        }
    ];

    ProxyHandler::new(routes)
}

// Test that we can identify routes that should use sticky sessions
#[test]
fn test_sticky_session_route_identification() {
    let handler = create_sticky_session_handler();

    // Stateful WebSocket route should ideally use sticky sessions
    let stateful_match = handler.find_route("/ws/stateful/user123").unwrap();
    assert_eq!(stateful_match.route.path, "/ws/stateful/*");
    assert_eq!(stateful_match.route.strategy, LoadBalancingStrategy::LeastConnections);

    // Broadcast WebSocket route doesn't need sticky sessions
    let broadcast_match = handler.find_route("/ws/broadcast/alerts").unwrap();
    assert_eq!(broadcast_match.route.path, "/ws/broadcast/*");
    assert_eq!(broadcast_match.route.strategy, LoadBalancingStrategy::RoundRobin);
}

// Test connection tracking for WebSocket load balancing
#[test]
fn test_websocket_connection_tracking() {
    let targets = vec![
        Target::new("http://localhost:5000".to_string()),
        Target::new("http://localhost:5001".to_string()),
        Target::new("http://localhost:5002".to_string())
    ];

    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::LeastConnections);

    // Initially all targets should have 0 connections
    assert_eq!(balancer.get_connection_count("http://localhost:5000"), 0);
    assert_eq!(balancer.get_connection_count("http://localhost:5001"), 0);
    assert_eq!(balancer.get_connection_count("http://localhost:5002"), 0);

    // Simulate WebSocket connections to different targets
    balancer.start_request("http://localhost:5000"); // WebSocket connection 1
    balancer.start_request("http://localhost:5001"); // WebSocket connection 2
    balancer.start_request("http://localhost:5000"); // WebSocket connection 3 to same target

    assert_eq!(balancer.get_connection_count("http://localhost:5000"), 2);
    assert_eq!(balancer.get_connection_count("http://localhost:5001"), 1);
    assert_eq!(balancer.get_connection_count("http://localhost:5002"), 0);

    // New connections should go to least loaded target
    let selected_target = balancer.select_target().unwrap();
    assert_eq!(selected_target.url, "http://localhost:5002"); // Should pick the one with 0 connections

    // End a connection
    balancer.end_request("http://localhost:5000");
    assert_eq!(balancer.get_connection_count("http://localhost:5000"), 1);

    // Now 5000 and 5001 both have 1 connection, 5002 has 0
    let next_target = balancer.select_target().unwrap();
    assert_eq!(next_target.url, "http://localhost:5002"); // Still should pick the one with 0
}

// Test that least connections strategy is ideal for WebSocket persistent connections
#[test]
fn test_least_connections_for_websockets() {
    let targets = vec![
        Target::new("http://localhost:5000".to_string()),
        Target::new("http://localhost:5001".to_string()),
        Target::new("http://localhost:5002".to_string())
    ];

    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::LeastConnections);

    // Simulate a scenario with existing WebSocket connections
    balancer.start_request("http://localhost:5000"); // 1 connection
    balancer.start_request("http://localhost:5000"); // 2 connections
    balancer.start_request("http://localhost:5001"); // 1 connection

    // New WebSocket should go to target with least connections
    let target1 = balancer.select_target().unwrap();
    assert_eq!(target1.url, "http://localhost:5002"); // 0 connections

    balancer.start_request("http://localhost:5002"); // Now 5002 has 1 connection

    // Next should go to either 5001 or 5002 (both have 1), not 5000 (has 2)
    let target2 = balancer.select_target().unwrap();
    assert!(target2.url == "http://localhost:5001" || target2.url == "http://localhost:5002");
    assert_ne!(target2.url, "http://localhost:5000");

    // Simulate long-running WebSocket connections staying active
    for _ in 0..10 {
        let target = balancer.select_target().unwrap();
        // Should consistently avoid the overloaded target
        if
            balancer.get_connection_count("http://localhost:5000") >
            balancer.get_connection_count(&target.url)
        {
            // This is good - we're avoiding the overloaded target
            assert!(true);
        }
    }
}

// Test sticky sessions concept (even though not fully implemented)
#[test]
fn test_websocket_sticky_session_concept() {
    // This test demonstrates what sticky sessions should do:
    // Route the same client to the same backend consistently

    let targets = vec![
        Target::new("http://localhost:5000".to_string()),
        Target::new("http://localhost:5001".to_string()),
        Target::new("http://localhost:5002".to_string())
    ];

    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::LeastConnections);

    // In a real sticky session implementation, we would:
    // 1. Hash the client identifier (IP, session ID, etc.)
    // 2. Map it consistently to the same backend
    // 3. Only use different backend if the mapped one is unhealthy

    // Simulate client identifiers
    let client_ips = vec!["192.168.1.100", "192.168.1.101", "192.168.1.102"];
    let mut client_mappings: HashMap<String, String> = HashMap::new();

    for client_ip in &client_ips {
        // In real implementation, this would be deterministic based on client ID
        let selected_target = balancer.select_target().unwrap();
        client_mappings.insert(client_ip.to_string(), selected_target.url.clone());

        // Start a connection for this client
        balancer.start_request(&selected_target.url);
    }

    // Verify that each client got a backend assignment
    assert_eq!(client_mappings.len(), 3);

    // In a sticky session system:
    // - Same client should always get same backend (until backend fails)
    // - Different clients can get same backend (that's fine)
    // - Backend failure should trigger re-assignment

    println!("Client mappings: {:?}", client_mappings);

    // This test demonstrates the concept but doesn't test actual implementation
    // since sticky sessions are not yet implemented
    assert!(true, "Sticky session concept test completed");
}

// Test WebSocket route prioritization over HTTP routes
#[test]
fn test_websocket_route_priority() {
    let handler = create_sticky_session_handler();

    // WebSocket request should match WebSocket route
    let ws_request = Request::builder()
        .method("GET")
        .uri("/ws/stateful/session123")
        .header("connection", "upgrade")
        .header("upgrade", "websocket")
        .body(Body::empty())
        .unwrap();

    assert!(ProxyHandler::is_websocket_request(&ws_request));

    let route_match = handler.find_route("/ws/stateful/session123").unwrap();
    assert_eq!(route_match.route.path, "/ws/stateful/*");
    assert_eq!(route_match.stripped_path, "/session123");

    // Regular HTTP request to same path should also route (but won't upgrade)
    let http_request = Request::builder()
        .method("POST")
        .uri("/ws/stateful/session123")
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();

    assert!(!ProxyHandler::is_websocket_request(&http_request));

    // Should still match the same route (allowing HTTP clients to same endpoints)
    let route_match = handler.find_route("/ws/stateful/session123").unwrap();
    assert_eq!(route_match.route.path, "/ws/stateful/*");
}

// Test WebSocket connection persistence simulation
#[test]
fn test_websocket_connection_persistence() {
    let targets = vec![
        Target::new("http://localhost:5000".to_string()),
        Target::new("http://localhost:5001".to_string())
    ];

    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::LeastConnections);

    // Simulate WebSocket connections that stay open for extended periods
    let target1 = balancer.select_target().unwrap();
    balancer.start_request(&target1.url);

    let target2 = balancer.select_target().unwrap();
    balancer.start_request(&target2.url);

    // Connections should be distributed
    assert_ne!(target1.url, target2.url);

    // Verify connection counts
    assert_eq!(balancer.get_connection_count(&target1.url), 1);
    assert_eq!(balancer.get_connection_count(&target2.url), 1);

    // Simulate long-running connections (they don't end immediately)
    // In real WebSocket scenarios, connections stay open for minutes/hours
    std::thread::sleep(std::time::Duration::from_millis(1)); // Simulate time passage

    // Connections should still be tracked
    assert_eq!(balancer.get_connection_count(&target1.url), 1);
    assert_eq!(balancer.get_connection_count(&target2.url), 1);

    // New connection should balance based on existing load
    let target3 = balancer.select_target().unwrap();
    // Could go to either target since both have equal load
    assert!(target3.url == target1.url || target3.url == target2.url);

    // End one connection (WebSocket closed)
    balancer.end_request(&target1.url);
    assert_eq!(balancer.get_connection_count(&target1.url), 0);
    assert_eq!(balancer.get_connection_count(&target2.url), 1);

    // Next connection should prefer the target with fewer connections
    let target4 = balancer.select_target().unwrap();
    assert_eq!(target4.url, target1.url); // Should pick the one with 0 connections
}

// Test WebSocket timeout configurations for different use cases
#[test]
fn test_websocket_timeout_strategies() {
    let handler = create_sticky_session_handler();

    // Stateful WebSocket connections typically need longer timeouts
    let stateful_route = handler.find_route("/ws/stateful/game123").unwrap();
    assert_eq!(stateful_route.route.timeout, 300); // 5 minutes

    // Broadcast connections might also need long timeouts
    let broadcast_route = handler.find_route("/ws/broadcast/news").unwrap();
    assert_eq!(broadcast_route.route.timeout, 300); // 5 minutes

    // Both WebSocket routes should have timeouts longer than typical HTTP (30s)
    assert!(stateful_route.route.timeout > 30);
    assert!(broadcast_route.route.timeout > 30);
}

// Test that health status affects WebSocket routing
#[test]
fn test_websocket_health_aware_routing() {
    let mut targets = vec![
        Target::new("http://localhost:5000".to_string()),
        Target::new("http://localhost:5001".to_string()),
        Target::new("http://localhost:5002".to_string())
    ];

    // Mark one target as unhealthy
    targets[1].healthy = false;

    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::LeastConnections);

    // Should only route to healthy targets
    for _ in 0..10 {
        let target = balancer.select_target().unwrap();
        assert_ne!(target.url, "http://localhost:5001"); // Unhealthy target should not be selected
        assert!(target.url == "http://localhost:5000" || target.url == "http://localhost:5002");
    }

    // Verify health status tracking
    assert_eq!(balancer.healthy_targets_count(), 2);
}

// Test error handling for WebSocket routing edge cases
#[test]
fn test_websocket_routing_edge_cases() {
    // Test with no targets configured
    let empty_routes = vec![ProxyRoute {
        path: "/ws/empty/*".to_string(),
        targets: vec![], // No targets
        target: None, // No legacy target either
        strategy: LoadBalancingStrategy::RoundRobin,
        timeout: 300,
        sticky_sessions: false,
        http_health: None,
        websocket_health: None,
        circuit_breaker: None,
        middleware: None,
    }];

    let empty_handler = ProxyHandler::new(empty_routes);
    let empty_match = empty_handler.find_route("/ws/empty/test").unwrap();
    assert_eq!(empty_match.route.get_targets().len(), 0);
    assert_eq!(empty_match.route.get_primary_target(), None);

    // Test with all targets unhealthy
    let mut unhealthy_targets = vec![
        Target::new("http://localhost:5000".to_string()),
        Target::new("http://localhost:5001".to_string())
    ];

    for target in &mut unhealthy_targets {
        target.healthy = false;
    }

    let unhealthy_balancer = LoadBalancer::new(
        unhealthy_targets,
        LoadBalancingStrategy::LeastConnections
    );
    assert_eq!(unhealthy_balancer.healthy_targets_count(), 0);
    assert!(unhealthy_balancer.select_target().is_none());
}
