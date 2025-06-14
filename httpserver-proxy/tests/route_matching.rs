use httpserver_proxy::RouteMatcher;
use httpserver_config::{ ProxyRoute, LoadBalancingStrategy };

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
        ssl: None,
    }
}

#[test]
fn test_exact_path_matching() {
    let routes = vec![
        create_test_route("/health", "http://localhost:3000"),
        create_test_route("/status", "http://localhost:3001")
    ];

    let matcher = RouteMatcher::new(routes);

    // Test exact matches
    let result = matcher.find_match("/health").unwrap();
    assert_eq!(result.route.target, Some("http://localhost:3000".to_string()));
    assert_eq!(result.stripped_path, "");
    assert!(!result.is_wildcard);

    let result = matcher.find_match("/status").unwrap();
    assert_eq!(result.route.target, Some("http://localhost:3001".to_string()));
    assert_eq!(result.stripped_path, "");
    assert!(!result.is_wildcard);

    // Test non-matches
    assert!(matcher.find_match("/unknown").is_none());
    assert!(matcher.find_match("/health/extra").is_none());
}

#[test]
fn test_wildcard_path_matching() {
    let routes = vec![
        create_test_route("/api/*", "http://localhost:3000"),
        create_test_route("/admin/*", "http://localhost:3001")
    ];

    let matcher = RouteMatcher::new(routes);

    // Test wildcard matches
    let result = matcher.find_match("/api/users").unwrap();
    assert_eq!(result.route.target, Some("http://localhost:3000".to_string()));
    assert_eq!(result.stripped_path, "/users");
    assert!(result.is_wildcard);

    let result = matcher.find_match("/api/users/123").unwrap();
    assert_eq!(result.route.target, Some("http://localhost:3000".to_string()));
    assert_eq!(result.stripped_path, "/users/123");
    assert!(result.is_wildcard);

    let result = matcher.find_match("/admin/dashboard").unwrap();
    assert_eq!(result.route.target, Some("http://localhost:3001".to_string()));
    assert_eq!(result.stripped_path, "/dashboard");
    assert!(result.is_wildcard);

    // Test prefix-only matches
    let result = matcher.find_match("/api").unwrap();
    assert_eq!(result.route.target, Some("http://localhost:3000".to_string()));
    assert_eq!(result.stripped_path, "");
    assert!(result.is_wildcard);

    // Test non-matches
    assert!(matcher.find_match("/other/path").is_none());
}

#[test]
fn test_route_priority() {
    let routes = vec![
        create_test_route("/api/health", "http://localhost:3000"), // Exact match
        create_test_route("/api/*", "http://localhost:3001") // Wildcard match
    ];

    let matcher = RouteMatcher::new(routes);

    // Exact match should win over wildcard due to order
    let result = matcher.find_match("/api/health").unwrap();
    assert_eq!(result.route.target, Some("http://localhost:3000".to_string()));
    assert!(!result.is_wildcard);

    // Wildcard should match other paths
    let result = matcher.find_match("/api/users").unwrap();
    assert_eq!(result.route.target, Some("http://localhost:3001".to_string()));
    assert!(result.is_wildcard);
}

#[test]
fn test_path_normalization() {
    let routes = vec![create_test_route("/api/*", "http://localhost:3000")];

    let matcher = RouteMatcher::new(routes);

    // Test paths with and without leading slash
    let result1 = matcher.find_match("/api/users").unwrap();
    let result2 = matcher.find_match("api/users").unwrap();

    assert_eq!(result1.route.target, result2.route.target);
    assert_eq!(result1.stripped_path, result2.stripped_path);
}

#[test]
fn test_global_wildcard() {
    let routes = vec![create_test_route("*", "http://localhost:3000")];

    let matcher = RouteMatcher::new(routes);

    // Test that global wildcard matches everything
    let result = matcher.find_match("/any/path").unwrap();
    assert_eq!(result.route.target, Some("http://localhost:3000".to_string()));
    assert_eq!(result.stripped_path, "/any/path");
    assert!(result.is_wildcard);

    let result = matcher.find_match("/").unwrap();
    assert_eq!(result.route.target, Some("http://localhost:3000".to_string()));
    assert_eq!(result.stripped_path, "/");
    assert!(result.is_wildcard);
}

#[test]
fn test_empty_routes() {
    let routes = vec![];
    let matcher = RouteMatcher::new(routes);

    assert!(matcher.find_match("/any/path").is_none());
}
