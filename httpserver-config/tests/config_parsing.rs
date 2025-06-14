// Configuration parsing and validation tests
use httpserver_config::{
    Config,
    Args,
    ProxyRoute,
    StaticConfig,
    HttpHealthConfig,
    WebSocketHealthConfig,
    LoggingConfig,
    ApplicationConfig,
    ServerConfig,
};
use httpserver_balancer::{ Target, LoadBalancingStrategy };
use std::path::PathBuf;
use tempfile::TempDir;
use std::fs;

/// Create a temporary directory with test files
fn create_test_directory() -> TempDir {
    let temp_dir = TempDir::new().unwrap();

    // Create a test index.html file
    let index_path = temp_dir.path().join("index.html");
    fs::write(&index_path, "<html><body>Test</body></html>").unwrap();

    temp_dir
}

/// Create a test configuration file
fn create_test_config_file(temp_dir: &TempDir, content: &str) -> PathBuf {
    let config_path = temp_dir.path().join("test_config.toml");
    fs::write(&config_path, content).unwrap();
    config_path
}

#[test]
fn test_default_config() {
    let config = Config::default();

    assert_eq!(config.static_config.directory, PathBuf::from("."));
    assert_eq!(config.static_config.fallback, "index.html");
    assert!(config.proxy.is_empty());
}

#[test]
fn test_config_from_args_no_config_file() {
    let temp_dir = create_test_directory();

    let args = Args {
        directory: temp_dir.path().to_path_buf(),
        port: 8080,
        config: None,
    };

    let config = Config::from_args(args).unwrap();

    assert_eq!(config.static_config.directory, temp_dir.path());
    assert_eq!(config.static_config.fallback, "index.html");
    assert!(config.proxy.is_empty());
}

#[test]
fn test_config_from_toml_file() {
    let temp_dir = create_test_directory();
    let toml_content = format!(
        r#"[static_config]
directory = "{}"
fallback = "app.html"

[[proxy]]
path = "/api/*"
target = "http://localhost:3000"
timeout = 45
"#,
        temp_dir.path().to_string_lossy().replace('\\', "/")
    );

    let config_path = create_test_config_file(&temp_dir, &toml_content);

    let config = Config::load_from_file(&config_path).unwrap();

    assert_eq!(config.static_config.directory, temp_dir.path());
    assert_eq!(config.static_config.fallback, "app.html");
    assert_eq!(config.proxy.len(), 1);
    assert_eq!(config.proxy[0].path, "/api/*");
    assert_eq!(config.proxy[0].get_primary_target().unwrap(), "http://localhost:3000");
    assert_eq!(config.proxy[0].timeout, 45);
}

#[test]
fn test_config_with_multiple_targets() {
    let temp_dir = create_test_directory();
    let toml_content = format!(
        r#"
[static_config]
directory = "{}"

[[proxy]]
path = "/api/*"
targets = [
    {{ url = "http://localhost:3000", weight = 1 }},
    {{ url = "http://localhost:3001", weight = 1 }},
    {{ url = "http://localhost:3002", weight = 1 }}
]
strategy = "round_robin"
timeout = 30
"#,
        temp_dir.path().to_string_lossy().replace('\\', "/")
    );

    let config_path = create_test_config_file(&temp_dir, &toml_content);

    let config = Config::load_from_file(&config_path).unwrap();

    assert_eq!(config.proxy.len(), 1);
    let route = &config.proxy[0];

    assert_eq!(route.path, "/api/*");
    assert_eq!(route.strategy, LoadBalancingStrategy::RoundRobin);
    assert_eq!(route.timeout, 30);

    let targets = route.get_targets();
    assert_eq!(targets.len(), 3);
    assert_eq!(targets[0].url, "http://localhost:3000");
    assert_eq!(targets[1].url, "http://localhost:3001");
    assert_eq!(targets[2].url, "http://localhost:3002");

    assert!(route.has_multiple_targets());
}

#[test]
fn test_config_with_health_checks() {
    let temp_dir = create_test_directory();
    let toml_content = format!(
        r#"
[static_config]
directory = "{}"

[[proxy]]
path = "/api/*"
targets = [{{ url = "http://localhost:3000", weight = 1 }}]

[proxy.http_health]
interval = 15
timeout = 3
path = "/health"
expected_status_codes = [200, 204]

[[proxy]]
path = "/ws/*"
targets = [{{ url = "http://localhost:5000", weight = 1 }}]

[proxy.websocket_health]
interval = 30
timeout = 5
path = "/ping"
ping_message = "health_check"
"#,
        temp_dir.path().to_string_lossy().replace('\\', "/")
    );

    let config_path = create_test_config_file(&temp_dir, &toml_content);

    let config = Config::load_from_file(&config_path).unwrap();

    assert_eq!(config.proxy.len(), 2);

    // Test HTTP health check
    let http_route = &config.proxy[0];
    assert!(http_route.http_health.is_some());
    let http_health = http_route.http_health.as_ref().unwrap();
    assert_eq!(http_health.interval, 15);
    assert_eq!(http_health.timeout, 3);
    assert_eq!(http_health.path, "/health");
    assert_eq!(http_health.expected_status_codes, vec![200, 204]);

    // Test WebSocket health check
    let ws_route = &config.proxy[1];
    assert!(ws_route.websocket_health.is_some());
    let ws_health = ws_route.websocket_health.as_ref().unwrap();
    assert_eq!(ws_health.interval, 30);
    assert_eq!(ws_health.timeout, 5);
    assert_eq!(ws_health.path, "/ping");
    assert_eq!(ws_health.ping_message, "health_check");
}

#[test]
fn test_config_validation_invalid_directory() {
    let config = Config {
        static_config: StaticConfig {
            directory: PathBuf::from("/nonexistent/directory"),
            fallback: "index.html".to_string(),
        },
        proxy: Vec::new(),
        logging: LoggingConfig::default(),
        application: ApplicationConfig::default(),
        server: ServerConfig::default(),
    };

    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("does not exist"));
}

#[test]
fn test_config_validation_invalid_proxy_route() {
    let temp_dir = create_test_directory();
    let config = Config {
        static_config: StaticConfig {
            directory: temp_dir.path().to_path_buf(),
            fallback: "index.html".to_string(),
        },        proxy: vec![ProxyRoute {
            path: "".to_string(), // Invalid empty path
            target: Some("http://localhost:3000".to_string()),
            targets: Vec::new(),
            strategy: LoadBalancingStrategy::RoundRobin,
            timeout: 30,
            sticky_sessions: false,
            http_health: None,
            websocket_health: None,
            ssl: None,
            circuit_breaker: None,
            middleware: None,
        }],
        logging: LoggingConfig::default(),
        application: ApplicationConfig::default(),
        server: ServerConfig::default(),
    };

    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("path cannot be empty"));
}

#[test]
fn test_config_validation_no_targets() {
    let temp_dir = create_test_directory();
    let config = Config {
        static_config: StaticConfig {
            directory: temp_dir.path().to_path_buf(),
            fallback: "index.html".to_string(),
        },        proxy: vec![ProxyRoute {
            path: "/api/*".to_string(),
            target: None, // No single target
            targets: Vec::new(), // No multiple targets
            strategy: LoadBalancingStrategy::RoundRobin,
            timeout: 30,
            sticky_sessions: false,
            http_health: None,
            websocket_health: None,
            ssl: None,
            circuit_breaker: None,
            middleware: None,
        }],
        logging: LoggingConfig::default(),
        application: ApplicationConfig::default(),
        server: ServerConfig::default(),
    };

    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must have at least one target"));
}

#[test]
fn test_config_validation_invalid_url() {
    let temp_dir = create_test_directory();
    let config = Config {
        static_config: StaticConfig {
            directory: temp_dir.path().to_path_buf(),
            fallback: "index.html".to_string(),
        },        proxy: vec![ProxyRoute {
            path: "/api/*".to_string(),
            target: Some("invalid-url".to_string()), // Invalid URL
            targets: Vec::new(),
            strategy: LoadBalancingStrategy::RoundRobin,
            timeout: 30,
            sticky_sessions: false,
            http_health: None,
            websocket_health: None,
            ssl: None,
            circuit_breaker: None,
            middleware: None,
        }],
        logging: LoggingConfig::default(),
        application: ApplicationConfig::default(),
        server: ServerConfig::default(),
    };

    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must be a valid HTTP/HTTPS URL"));
}

#[test]
fn test_proxy_route_legacy_vs_new_targets() {
    // Test legacy single target
    let legacy_route = ProxyRoute {
        path: "/legacy/*".to_string(),
        target: Some("http://localhost:8000".to_string()),
        targets: Vec::new(),
        strategy: LoadBalancingStrategy::RoundRobin,
        timeout: 30,
        sticky_sessions: false,
        http_health: None,
        websocket_health: None,
        circuit_breaker: None,
        middleware: None,
        ssl: None,
    };

    assert_eq!(legacy_route.get_primary_target().unwrap(), "http://localhost:8000");
    assert_eq!(legacy_route.get_targets().len(), 1);
    assert!(!legacy_route.has_multiple_targets());

    // Test new multiple targets
    let new_route = ProxyRoute {
        path: "/api/*".to_string(),
        target: None,
        targets: vec![
            Target::new("http://localhost:3000".to_string()),
            Target::new("http://localhost:3001".to_string())
        ],        strategy: LoadBalancingStrategy::RoundRobin,
        timeout: 30,
        sticky_sessions: false,
        http_health: None,
        websocket_health: None,
        circuit_breaker: None,
        middleware: None,
        ssl: None,
    };

    assert_eq!(new_route.get_primary_target().unwrap(), "http://localhost:3000");
    assert_eq!(new_route.get_targets().len(), 2);
    assert!(new_route.has_multiple_targets());
}

#[test]
fn test_health_config_defaults() {
    let http_health = HttpHealthConfig {
        interval: 30,
        timeout: 5,
        path: "/health".to_string(),
        expected_status_codes: vec![200],
    };

    assert_eq!(http_health.interval, 30);
    assert_eq!(http_health.timeout, 5);
    assert_eq!(http_health.path, "/health");
    assert_eq!(http_health.expected_status_codes, vec![200]);

    let ws_health = WebSocketHealthConfig {
        interval: 30,
        timeout: 5,
        path: "/health".to_string(),
        ping_message: "ping".to_string(),
    };

    assert_eq!(ws_health.interval, 30);
    assert_eq!(ws_health.timeout, 5);
    assert_eq!(ws_health.path, "/health");
    assert_eq!(ws_health.ping_message, "ping");
}
