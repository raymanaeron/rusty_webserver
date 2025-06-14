//! Tunnel Server Tests
//! Tests for Phase 7.2 Tunnel Server functionality including subdomain management,
//! traffic routing, authentication, and rate limiting.

use httpserver_tunnel::config::{TunnelServerConfig, TunnelServerAuthConfig, TunnelServerNetworkConfig, SubdomainStrategy};
use httpserver_tunnel::server::TunnelServer;
use httpserver_tunnel::protocol::{TunnelMessage, TunnelProtocol};
use tokio::time::{timeout, Duration};
use std::collections::HashMap;

/// Create test tunnel server configuration
fn create_test_server_config() -> TunnelServerConfig {
    TunnelServerConfig {
        enabled: true,
        tunnel_port: 8091,
        public_port: 8092,
        public_https_port: 8093,
        base_domain: "test.localhost".to_string(),
        max_tunnels: 10,
        subdomain_strategy: SubdomainStrategy::Random,
        auth: TunnelServerAuthConfig {
            required: true,
            api_keys: vec!["test-api-key".to_string(), "another-key".to_string()],
            jwt_secret: Some("test-secret".to_string()),
            token_expiry: 3600, // 1 hour
        },
        network: TunnelServerNetworkConfig {
            bind_address: "127.0.0.1".to_string(),
            public_bind_address: "127.0.0.1".to_string(),
            ipv6_enabled: false,
            tcp_keepalive: true,
            tcp_keepalive_idle: 600,
            tcp_keepalive_interval: 60,
            tcp_keepalive_probes: 9,
            socket_reuse_address: true,
            socket_reuse_port: false,
        },
        ..Default::default()
    }
}

#[tokio::test]
async fn test_tunnel_server_creation() {
    let config = create_test_server_config();
    
    // Should create server successfully
    let server = TunnelServer::new(config.clone());
    assert!(server.is_ok(), "Failed to create tunnel server");
    
    let _server = server.unwrap();
    // Server should be created with proper internal state
    // Note: We can't access private fields, but creation success indicates proper initialization
}

#[tokio::test]
async fn test_tunnel_server_disabled() {
    let mut config = create_test_server_config();
    config.enabled = false;
    
    let server = TunnelServer::new(config).unwrap();
    
    // Starting disabled server should return immediately without error
    let result = timeout(Duration::from_millis(100), server.start()).await;
    assert!(result.is_ok(), "Disabled server should start quickly");
    assert!(result.unwrap().is_ok(), "Disabled server should not error");
}

#[tokio::test]
async fn test_subdomain_strategy_random() {
    let config = create_test_server_config();
    assert!(matches!(config.subdomain_strategy, SubdomainStrategy::Random));
    
    // Test subdomain strategy configuration
    let mut config_uuid = config.clone();
    config_uuid.subdomain_strategy = SubdomainStrategy::Uuid;
    
    let mut config_user = config.clone();
    config_user.subdomain_strategy = SubdomainStrategy::UserSpecified;
    
    // All strategies should be valid for server creation
    assert!(TunnelServer::new(config).is_ok());
    assert!(TunnelServer::new(config_uuid).is_ok());
    assert!(TunnelServer::new(config_user).is_ok());
}

#[tokio::test]
async fn test_authentication_configuration() {
    let config = create_test_server_config();
    
    // Test that authentication config is properly structured
    assert!(config.auth.required);
    assert_eq!(config.auth.api_keys.len(), 2);
    assert!(config.auth.api_keys.contains(&"test-api-key".to_string()));
    assert!(config.auth.api_keys.contains(&"another-key".to_string()));
    assert_eq!(config.auth.jwt_secret, Some("test-secret".to_string()));
    assert_eq!(config.auth.token_expiry, 3600);
    
    // Test no-auth configuration
    let mut no_auth_config = config;
    no_auth_config.auth.required = false;
    no_auth_config.auth.api_keys.clear();
    
    let server = TunnelServer::new(no_auth_config);
    assert!(server.is_ok(), "Should accept no-auth configuration");
}

#[tokio::test]
async fn test_rate_limiting_configuration() {
    let config = create_test_server_config();
    
    // Test rate limiting defaults
    assert!(config.rate_limiting.enabled);
    assert_eq!(config.rate_limiting.requests_per_minute, 1000);
    assert_eq!(config.rate_limiting.max_concurrent_connections, 100);
    assert_eq!(config.rate_limiting.max_bandwidth_bps, 10_485_760); // 10MB/s
    
    // Test custom rate limiting
    let mut custom_config = config;
    custom_config.rate_limiting.requests_per_minute = 500;
    custom_config.rate_limiting.max_concurrent_connections = 50;
    custom_config.rate_limiting.max_bandwidth_bps = 5_242_880; // 5MB/s
    
    let server = TunnelServer::new(custom_config);
    assert!(server.is_ok(), "Should accept custom rate limiting");
}

#[tokio::test]
async fn test_network_configuration() {
    let config = create_test_server_config();
    
    // Test network configuration
    assert_eq!(config.network.bind_address, "127.0.0.1");
    assert_eq!(config.network.public_bind_address, "127.0.0.1");
    assert!(!config.network.ipv6_enabled);
    assert!(config.network.tcp_keepalive);
    assert_eq!(config.network.tcp_keepalive_idle, 600);
    assert_eq!(config.network.tcp_keepalive_interval, 60);
    assert_eq!(config.network.tcp_keepalive_probes, 9);
    assert!(config.network.socket_reuse_address);
    assert!(!config.network.socket_reuse_port);
    
    // Test IPv6 configuration
    let mut ipv6_config = config;
    ipv6_config.network.ipv6_enabled = true;
    ipv6_config.network.bind_address = "::1".to_string();
    ipv6_config.network.public_bind_address = "::1".to_string();
    
    let server = TunnelServer::new(ipv6_config);
    assert!(server.is_ok(), "Should accept IPv6 configuration");
}

#[tokio::test]
async fn test_ssl_configuration() {
    let config = create_test_server_config();
    
    // Test SSL configuration defaults (struct defaults, not serde defaults)
    assert!(!config.ssl.enabled); // Default trait uses false for bool
    assert!(!config.ssl.redirect_http); // Default trait uses false for bool
    assert!(config.ssl.wildcard_cert_file.is_none());
    assert!(config.ssl.wildcard_key_file.is_none());
    
    // Test custom SSL configuration
    let mut ssl_config = config;
    ssl_config.ssl.enabled = true;
    ssl_config.ssl.wildcard_cert_file = Some("/path/to/wildcard.crt".into());
    ssl_config.ssl.wildcard_key_file = Some("/path/to/wildcard.key".into());
    ssl_config.ssl.redirect_http = true;
    
    let server = TunnelServer::new(ssl_config);
    assert!(server.is_ok(), "Should accept custom SSL configuration");
}

#[tokio::test]
async fn test_tunnel_protocol_messages() {
    // Test authentication message creation
    let auth_msg = TunnelProtocol::create_auth_message("test-token", Some("myapp"));
    match auth_msg {
        TunnelMessage::Auth { token, subdomain, protocol_version } => {
            assert_eq!(token, "test-token");
            assert_eq!(subdomain, Some("myapp".to_string()));
            assert_eq!(protocol_version, "1.0");
        }
        _ => panic!("Expected Auth message"),
    }
    
    // Test HTTP request message
    let mut headers = HashMap::new();
    headers.insert("host".to_string(), "example.com".to_string());
    headers.insert("user-agent".to_string(), "test-agent".to_string());
    
    let request_msg = TunnelProtocol::create_http_request_message(
        "GET",
        "/api/test",
        headers.clone(),
        Some(b"test body".to_vec()),
        "127.0.0.1"
    );
    
    match request_msg {
        TunnelMessage::HttpRequest { method, path, headers: msg_headers, body, client_ip, .. } => {
            assert_eq!(method, "GET");
            assert_eq!(path, "/api/test");
            assert_eq!(msg_headers.get("host"), Some(&"example.com".to_string()));
            assert_eq!(msg_headers.get("user-agent"), Some(&"test-agent".to_string()));
            assert_eq!(body, Some(b"test body".to_vec()));
            assert_eq!(client_ip, "127.0.0.1");
        }
        _ => panic!("Expected HttpRequest message"),
    }
    
    // Test HTTP response message
    let response_msg = TunnelProtocol::create_http_response_message(
        "request-123",
        200,
        headers,
        Some(b"response body".to_vec())
    );
    
    match response_msg {
        TunnelMessage::HttpResponse { id, status, headers: resp_headers, body } => {
            assert_eq!(id, "request-123");
            assert_eq!(status, 200);
            assert_eq!(resp_headers.get("host"), Some(&"example.com".to_string()));
            assert_eq!(body, Some(b"response body".to_vec()));
        }
        _ => panic!("Expected HttpResponse message"),
    }
}

#[tokio::test]
async fn test_message_serialization() {
    let _protocol = TunnelProtocol::new();
    
    // Test ping message serialization
    let ping = TunnelProtocol::create_ping_message();
    let serialized = TunnelProtocol::serialize_message(&ping).unwrap();
    let deserialized = TunnelProtocol::deserialize_message(&serialized).unwrap();
    
    match (ping, deserialized) {
        (TunnelMessage::Ping { timestamp: ts1 }, TunnelMessage::Ping { timestamp: ts2 }) => {
            assert_eq!(ts1, ts2);
        }
        _ => panic!("Ping serialization/deserialization failed"),
    }
    
    // Test error message serialization
    let error_msg = TunnelProtocol::create_error_message(404, "Not found");
    let serialized = TunnelProtocol::serialize_message(&error_msg).unwrap();
    let deserialized = TunnelProtocol::deserialize_message(&serialized).unwrap();
    
    match (error_msg, deserialized) {
        (TunnelMessage::Error { code: c1, message: m1 }, TunnelMessage::Error { code: c2, message: m2 }) => {
            assert_eq!(c1, c2);
            assert_eq!(m1, m2);
        }
        _ => panic!("Error serialization/deserialization failed"),
    }
}

#[tokio::test]
async fn test_protocol_version_compatibility() {
    let protocol = TunnelProtocol::new();
    
    // Test compatible version
    assert!(protocol.is_compatible_version("1.0"));
    
    // Test incompatible versions
    assert!(!protocol.is_compatible_version("2.0"));
    assert!(!protocol.is_compatible_version("0.9"));
    assert!(!protocol.is_compatible_version("1.1"));
    assert!(!protocol.is_compatible_version("invalid"));
}

#[tokio::test]
async fn test_server_configuration_limits() {
    let mut config = create_test_server_config();
    
    // Test maximum tunnels limit
    config.max_tunnels = 5000;
    assert!(TunnelServer::new(config.clone()).is_ok());
    
    config.max_tunnels = 0;
    assert!(TunnelServer::new(config.clone()).is_ok());
    
    // Test base domain validation (server creation should not validate domain format)
    config.base_domain = "".to_string();
    assert!(TunnelServer::new(config.clone()).is_ok());
    
    config.base_domain = "httpserver.io".to_string();
    assert!(TunnelServer::new(config.clone()).is_ok());
    
    config.base_domain = "localhost".to_string();
    assert!(TunnelServer::new(config.clone()).is_ok());
}

#[tokio::test]
async fn test_port_configuration_edge_cases() {
    let mut config = create_test_server_config();
    
    // Test minimum valid ports
    config.tunnel_port = 1024;
    config.public_port = 1025;
    config.public_https_port = 1026;
    assert!(TunnelServer::new(config.clone()).is_ok());
    
    // Test maximum valid ports
    config.tunnel_port = 65535;
    config.public_port = 65534;
    config.public_https_port = 65533;
    assert!(TunnelServer::new(config.clone()).is_ok());
    
    // Test same ports (should be allowed at creation time)
    config.tunnel_port = 8080;
    config.public_port = 8080;
    config.public_https_port = 8080;
    assert!(TunnelServer::new(config).is_ok());
}
