use httpserver_proxy::ProxyHandler;
use httpserver_config::{ProxyRoute, LoadBalancingStrategy};

fn create_test_route(path: &str, target: &str) -> ProxyRoute {
    ProxyRoute {
        path: path.to_string(),
        target: Some(target.to_string()),
        targets: vec![],
        strategy: LoadBalancingStrategy::RoundRobin,
        timeout: 30,
        sticky_sessions: false,
        http_health: None,
        websocket_health: None,
        circuit_breaker: None,
        middleware: None,
    }
}

#[test]
fn test_proxy_handler() {
    let routes = vec![
        create_test_route("/api/*", "http://localhost:3000"),
        create_test_route("/health", "http://localhost:3001"),
    ];
    
    let handler = ProxyHandler::new(routes.clone());
    
    assert!(handler.has_routes());
    assert_eq!(handler.routes().len(), 2);
    
    let result = handler.find_route("/api/users").unwrap();
    assert_eq!(result.route.get_primary_target().unwrap(), "http://localhost:3000");
    assert_eq!(result.stripped_path, "/users");
    
    let result = handler.find_route("/health").unwrap();
    assert_eq!(result.route.get_primary_target().unwrap(), "http://localhost:3001");
    assert_eq!(result.stripped_path, "");
}

#[test]
fn test_empty_proxy_handler() {
    let routes = vec![];
    let handler = ProxyHandler::new(routes);
    
    assert!(!handler.has_routes());
    assert_eq!(handler.routes().len(), 0);
    assert!(handler.find_route("/any/path").is_none());
}
