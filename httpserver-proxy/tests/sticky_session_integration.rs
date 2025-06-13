// Simple sticky session integration test
use httpserver_proxy::ProxyHandler;
use httpserver_config::{ProxyRoute, LoadBalancingStrategy};
use httpserver_balancer::{Target, LoadBalancer};

#[test]
fn test_sticky_session_integration() {
    // Create a load balancer with sticky session support
    let targets = vec![
        Target::new("http://localhost:5000".to_string()),
        Target::new("http://localhost:5001".to_string()),
        Target::new("http://localhost:5002".to_string()),
    ];
    
    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::LeastConnections);
    
    // Test that sticky sessions work consistently
    let client_id = "192.168.1.100";
    
    // First selection - should pick a target
    let first_target = balancer.select_target_sticky(client_id).unwrap();
    let first_url = first_target.url.clone();
    
    // Second selection - should pick the same target
    let second_target = balancer.select_target_sticky(client_id).unwrap();
    let second_url = second_target.url.clone();
    
    assert_eq!(first_url, second_url, "Sticky sessions should route to same target");
    
    // Verify the sticky session is stored
    let sticky_target = balancer.get_sticky_target(client_id);
    assert!(sticky_target.is_some());
    assert_eq!(sticky_target.unwrap(), first_url);
    
    // Test with different client - should potentially get different target
    let client_id2 = "192.168.1.101";
    let different_target = balancer.select_target_sticky(client_id2).unwrap();
    
    // Should also have sticky session now
    let sticky_target2 = balancer.get_sticky_target(client_id2);
    assert!(sticky_target2.is_some());
    assert_eq!(sticky_target2.unwrap(), different_target.url);
}

#[test]
fn test_sticky_session_clear() {
    let targets = vec![
        Target::new("http://localhost:5000".to_string()),
        Target::new("http://localhost:5001".to_string()),
    ];
    
    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::RoundRobin);
    
    let client_id = "test_client";
    
    // Create sticky session
    let _target = balancer.select_target_sticky(client_id).unwrap();
    assert!(balancer.get_sticky_target(client_id).is_some());
    
    // Clear sticky session
    balancer.clear_sticky_session(client_id);
    assert!(balancer.get_sticky_target(client_id).is_none());
}

#[test]
fn test_proxy_handler_sticky_sessions() {    // Create proxy route with sticky sessions enabled
    let routes = vec![
        ProxyRoute {
            path: "/ws/chat/*".to_string(),
            targets: vec![
                Target::new("http://localhost:5000".to_string()),
                Target::new("http://localhost:5001".to_string()),
            ],
            target: None,
            strategy: LoadBalancingStrategy::LeastConnections,
            timeout: 300,
            sticky_sessions: true,
            http_health: None,
            websocket_health: None,
        }
    ];
    
    let handler = ProxyHandler::new(routes);
    
    // Verify route configuration
    let route_match = handler.find_route("/ws/chat/room1").unwrap();
    assert!(route_match.route.sticky_sessions, "Sticky sessions should be enabled");
    assert_eq!(route_match.route.strategy, LoadBalancingStrategy::LeastConnections);
}
