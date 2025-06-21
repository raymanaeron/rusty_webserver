//! Connection Management Tests
//! Tests for WebSocket connection management and auto-reconnection

use httpserver_tunnel::connection::{ConnectionState, ReconnectionStrategy};
use httpserver_tunnel::config::{TunnelEndpoint, TunnelAuthConfig, TokenRefreshConfig};
use std::collections::HashMap;
use std::time::Duration;

#[tokio::test]
async fn test_connection_state_enum() {
    // Test that all connection states can be created and compared
    let states = vec![
        ConnectionState::Disconnected,
        ConnectionState::Connecting,
        ConnectionState::Connected,
        ConnectionState::Authenticating,
        ConnectionState::Authenticated,
        ConnectionState::Reconnecting,
        ConnectionState::Failed("Test error".to_string()),
    ];

    for state in states {
        match state {
            ConnectionState::Disconnected => assert_eq!(state, ConnectionState::Disconnected),
            ConnectionState::Connecting => assert_eq!(state, ConnectionState::Connecting),
            ConnectionState::Connected => assert_eq!(state, ConnectionState::Connected),
            ConnectionState::Authenticating => assert_eq!(state, ConnectionState::Authenticating),
            ConnectionState::Authenticated => assert_eq!(state, ConnectionState::Authenticated),
            ConnectionState::Reconnecting => assert_eq!(state, ConnectionState::Reconnecting),            ConnectionState::Failed(ref _msg) => {
                if let ConnectionState::Failed(test_msg) = &state {
                    assert_eq!(test_msg, "Test error");
                }
            }
        }
    }
}

#[tokio::test]
async fn test_reconnection_strategy_exponential() {
    let strategy = ReconnectionStrategy::Exponential {
        initial_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(60),
        multiplier: 2.0,
        jitter_factor: 0.1,
    };

    // Test delay calculation for different attempts
    let delay1 = strategy.next_delay(0);
    let delay2 = strategy.next_delay(1);
    let delay3 = strategy.next_delay(5);

    // First attempt should be close to initial delay
    assert!(delay1.as_secs() <= 2); // With jitter, could be up to ~1.1 seconds

    // Second attempt should be larger
    assert!(delay2 > delay1);

    // After 5 attempts, should be close to or at max delay
    assert!(delay3.as_secs() <= 60);
}

#[tokio::test]
async fn test_reconnection_strategy_fixed() {
    let strategy = ReconnectionStrategy::Fixed {
        delay: Duration::from_secs(5),
    };

    // All attempts should return the same delay
    for attempt in 0..10 {
        let delay = strategy.next_delay(attempt);
        assert_eq!(delay, Duration::from_secs(5));
    }
}

#[tokio::test]
async fn test_reconnection_strategy_linear() {
    let strategy = ReconnectionStrategy::Linear {
        initial_delay: Duration::from_secs(1),
        increment: Duration::from_secs(2),
        max_delay: Duration::from_secs(10),
    };

    let delay0 = strategy.next_delay(0);
    let delay1 = strategy.next_delay(1);
    let delay2 = strategy.next_delay(2);
    let delay5 = strategy.next_delay(5);

    assert_eq!(delay0, Duration::from_secs(1)); // 1 + 2*0 = 1
    assert_eq!(delay1, Duration::from_secs(3)); // 1 + 2*1 = 3
    assert_eq!(delay2, Duration::from_secs(5)); // 1 + 2*2 = 5
    assert_eq!(delay5, Duration::from_secs(10)); // 1 + 2*5 = 11, capped at 10
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
async fn test_tunnel_endpoint_with_custom_domain() {
    let endpoint = TunnelEndpoint {
        server_url: "wss://tunnel.example.com/connect".to_string(),
        subdomain: None,
        custom_domain: Some("api.mycompany.com".to_string()),
        protocol_version: "1.1".to_string(),
        connection_timeout: 60,
        keepalive_interval: 15,
        max_connections: 50,
    };

    assert_eq!(endpoint.custom_domain, Some("api.mycompany.com".to_string()));
    assert_eq!(endpoint.subdomain, None);
    assert_eq!(endpoint.protocol_version, "1.1");
}

/// Helper function to create test auth config
fn create_test_auth_config() -> TunnelAuthConfig {
    TunnelAuthConfig {
        method: "api_key".to_string(),
        api_key: Some("test-key".to_string()),
        token: None,
        cert_file: None,
        key_file: None,
        user: None,
        headers: HashMap::new(),
        token_refresh: TokenRefreshConfig::default(),
    }
}

#[tokio::test]
async fn test_auth_config_creation() {
    let auth_config = create_test_auth_config();
    
    assert_eq!(auth_config.method, "api_key");
    assert_eq!(auth_config.api_key, Some("test-key".to_string()));
    assert_eq!(auth_config.token, None);
    assert_eq!(auth_config.cert_file, None);
    assert_eq!(auth_config.key_file, None);
    assert_eq!(auth_config.user, None);
    assert!(auth_config.headers.is_empty());
}

#[tokio::test]
async fn test_token_refresh_config() {
    let refresh_config = TokenRefreshConfig {
        enabled: true,
        refresh_url: Some("https://auth.example.com/refresh".to_string()),
        interval: 3600,
        refresh_token: Some("refresh-token-123".to_string()),
    };

    assert!(refresh_config.enabled);
    assert_eq!(refresh_config.refresh_url, Some("https://auth.example.com/refresh".to_string()));
    assert_eq!(refresh_config.interval, 3600);
    assert_eq!(refresh_config.refresh_token, Some("refresh-token-123".to_string()));
}
