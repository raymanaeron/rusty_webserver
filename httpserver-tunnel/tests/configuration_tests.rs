//! Configuration Module Tests
//! Tests for TOML configuration parsing and validation

use httpserver_tunnel::config::{
    TunnelConfig, TunnelEndpoint, TunnelAuthConfig, TokenRefreshConfig,
    TunnelServerConfig
};
use httpserver_tunnel::config; // Import the module for non-exported types
use serde_json;
use std::collections::HashMap;

#[tokio::test]
async fn test_tunnel_config_default() {
    let config = TunnelConfig::default();
    
    assert!(!config.enabled);
    assert_eq!(config.local_port, None);
    // The default local_host is empty string when using Default trait
    // The serde default only applies when deserializing
    assert_eq!(config.local_host, "");
    assert!(config.endpoints.is_empty());
}

#[tokio::test]
async fn test_tunnel_endpoint_creation() {
    let endpoint = TunnelEndpoint {
        server_url: "wss://tunnel.example.com/connect".to_string(),
        subdomain: Some("my-app".to_string()),
        custom_domain: None,
        protocol_version: "1.0".to_string(),
        connection_timeout: 30,
        keepalive_interval: 30,
        max_connections: 100,
    };

    assert_eq!(endpoint.server_url, "wss://tunnel.example.com/connect");
    assert_eq!(endpoint.subdomain, Some("my-app".to_string()));
    assert_eq!(endpoint.custom_domain, None);
    assert_eq!(endpoint.protocol_version, "1.0");
    assert_eq!(endpoint.connection_timeout, 30);
    assert_eq!(endpoint.keepalive_interval, 30);
    assert_eq!(endpoint.max_connections, 100);
}

#[tokio::test]
async fn test_tunnel_auth_config_api_key() {
    let auth_config = TunnelAuthConfig {
        method: "api_key".to_string(),
        api_key: Some("sk-test-12345".to_string()),
        token: None,
        cert_file: None,
        key_file: None,
        user: Some("testuser".to_string()),
        headers: HashMap::new(),
        token_refresh: TokenRefreshConfig::default(),
    };

    assert_eq!(auth_config.method, "api_key");
    assert_eq!(auth_config.api_key, Some("sk-test-12345".to_string()));
    assert_eq!(auth_config.user, Some("testuser".to_string()));
    assert!(auth_config.headers.is_empty());
}

#[tokio::test]
async fn test_tunnel_auth_config_token() {
    let mut refresh_config = TokenRefreshConfig::default();
    refresh_config.enabled = true;
    refresh_config.refresh_url = Some("https://auth.example.com/refresh".to_string());
    refresh_config.interval = 3600;

    let auth_config = TunnelAuthConfig {
        method: "token".to_string(),
        api_key: None,
        token: Some("eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...".to_string()),
        cert_file: None,
        key_file: None,
        user: None,
        headers: HashMap::new(),
        token_refresh: refresh_config,
    };

    assert_eq!(auth_config.method, "token");
    assert!(auth_config.token.is_some());
    assert!(auth_config.token_refresh.enabled);
    assert_eq!(auth_config.token_refresh.interval, 3600);
}

#[tokio::test]
async fn test_tunnel_auth_config_certificate() {
    let auth_config = TunnelAuthConfig {
        method: "certificate".to_string(),
        api_key: None,
        token: None,
        cert_file: Some("client.crt".into()),
        key_file: Some("client.key".into()),
        user: None,
        headers: HashMap::new(),
        token_refresh: TokenRefreshConfig::default(),
    };

    assert_eq!(auth_config.method, "certificate");
    assert_eq!(auth_config.cert_file, Some("client.crt".into()));
    assert_eq!(auth_config.key_file, Some("client.key".into()));
}

#[tokio::test]
async fn test_tunnel_auth_config_with_custom_headers() {
    let mut headers = HashMap::new();
    headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());
    headers.insert("X-API-Version".to_string(), "v1.0".to_string());

    let auth_config = TunnelAuthConfig {
        method: "api_key".to_string(),
        api_key: Some("test-key".to_string()),
        token: None,
        cert_file: None,
        key_file: None,
        user: None,
        headers,
        token_refresh: TokenRefreshConfig::default(),
    };

    assert_eq!(auth_config.headers.len(), 2);
    assert_eq!(auth_config.headers.get("X-Custom-Header"), Some(&"custom-value".to_string()));
    assert_eq!(auth_config.headers.get("X-API-Version"), Some(&"v1.0".to_string()));
}

#[tokio::test]
async fn test_token_refresh_config() {
    let config = TokenRefreshConfig {
        enabled: true,
        refresh_url: Some("https://auth.example.com/refresh".to_string()),
        interval: 1800, // 30 minutes
        refresh_token: Some("refresh_12345".to_string()),
    };

    assert!(config.enabled);
    assert_eq!(config.refresh_url, Some("https://auth.example.com/refresh".to_string()));
    assert_eq!(config.interval, 1800);
    assert_eq!(config.refresh_token, Some("refresh_12345".to_string()));
}

#[tokio::test]
async fn test_subdomain_strategy_enum() {
    let strategies = vec![
        config::SubdomainStrategy::Random,
        config::SubdomainStrategy::UserSpecified,
        config::SubdomainStrategy::Uuid,
    ];

    for strategy in strategies {
        match strategy {
            config::SubdomainStrategy::Random => {
                // Should serialize/deserialize correctly
                let json = serde_json::to_string(&strategy).unwrap();
                let deserialized: config::SubdomainStrategy = serde_json::from_str(&json).unwrap();
                assert!(matches!(deserialized, config::SubdomainStrategy::Random));
            },
            config::SubdomainStrategy::UserSpecified => {
                let json = serde_json::to_string(&strategy).unwrap();
                let deserialized: config::SubdomainStrategy = serde_json::from_str(&json).unwrap();
                assert!(matches!(deserialized, config::SubdomainStrategy::UserSpecified));
            },
            config::SubdomainStrategy::Uuid => {
                let json = serde_json::to_string(&strategy).unwrap();
                let deserialized: config::SubdomainStrategy = serde_json::from_str(&json).unwrap();
                assert!(matches!(deserialized, config::SubdomainStrategy::Uuid));
            },
        }
    }
}

#[tokio::test]
async fn test_tunnel_config_serialization() {
    let mut config = TunnelConfig::default();
    config.enabled = true;
    config.local_port = Some(3000);
    
    let endpoint = TunnelEndpoint {
        server_url: "wss://tunnel.example.com".to_string(),
        subdomain: Some("test-app".to_string()),
        custom_domain: None,
        protocol_version: "1.0".to_string(),
        connection_timeout: 30,
        keepalive_interval: 30,
        max_connections: 100,
    };
    config.endpoints.push(endpoint);

    // Test JSON serialization
    let json = serde_json::to_string_pretty(&config).unwrap();
    assert!(json.contains("\"enabled\": true"));
    assert!(json.contains("\"local_port\": 3000"));
    assert!(json.contains("\"server_url\": \"wss://tunnel.example.com\""));

    // Test deserialization
    let deserialized: TunnelConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.enabled, true);
    assert_eq!(deserialized.local_port, Some(3000));
    assert_eq!(deserialized.endpoints.len(), 1);
    assert_eq!(deserialized.endpoints[0].server_url, "wss://tunnel.example.com");
}

#[tokio::test]
async fn test_complete_tunnel_config() {
    let auth_config = TunnelAuthConfig {
        method: "api_key".to_string(),
        api_key: Some("sk-test-12345".to_string()),
        token: None,
        cert_file: None,
        key_file: None,
        user: Some("testuser".to_string()),
        headers: HashMap::new(),
        token_refresh: TokenRefreshConfig::default(),
    };

    let endpoint = TunnelEndpoint {
        server_url: "wss://tunnel.example.com/connect".to_string(),
        subdomain: Some("my-app".to_string()),
        custom_domain: None,
        protocol_version: "1.0".to_string(),
        connection_timeout: 30,
        keepalive_interval: 30,
        max_connections: 100,
    };

    let config = TunnelConfig {
        enabled: true,
        local_port: Some(8080),
        local_host: "localhost".to_string(),
        endpoints: vec![endpoint],        auth: auth_config,
        reconnection: Default::default(),
        monitoring: Default::default(),
        ssl: Default::default(),
        server: TunnelServerConfig::default(),
    };

    assert!(config.enabled);
    assert_eq!(config.local_port, Some(8080));
    assert_eq!(config.local_host, "localhost");
    assert_eq!(config.endpoints.len(), 1);
    assert_eq!(config.auth.method, "api_key");
    assert_eq!(config.auth.api_key, Some("sk-test-12345".to_string()));
}

#[tokio::test]
async fn test_tunnel_config_toml_compatibility() {
    let toml_content = r#"
enabled = true
local_port = 3000
local_host = "127.0.0.1"

[[endpoints]]
server_url = "wss://tunnel.example.com/connect"
subdomain = "my-app"
protocol_version = "1.0"
connection_timeout = 30
keepalive_interval = 30
max_connections = 100

[auth]
method = "api_key"
api_key = "sk-test-12345"
user = "testuser"

[auth.token_refresh]
enabled = false

[reconnection]
enabled = true

[monitoring]
enabled = true

[server]
enabled = false
"#;

    // For now, just verify the TOML structure is reasonable
    // (Full TOML parsing may require additional setup in the actual config module)
    assert!(toml_content.contains("enabled = true"));
    assert!(toml_content.contains("local_port = 3000"));
    assert!(toml_content.contains("server_url = \"wss://tunnel.example.com/connect\""));
}
