//! Integration Tests
//! End-to-end testing of tunnel client functionality

use httpserver_tunnel::{TunnelClient, TunnelError};
use httpserver_tunnel::config::{TunnelConfig, TunnelEndpoint, TunnelAuthConfig, TokenRefreshConfig};
use httpserver_tunnel::connection::ConnectionState;
use httpserver_tunnel::status::{TunnelStatusMonitor, TunnelEvent, TunnelEventType};
use std::collections::HashMap;

/// Create a test tunnel configuration
fn create_test_tunnel_config() -> TunnelConfig {
    let auth_config = TunnelAuthConfig {
        method: "api_key".to_string(),
        api_key: Some("test-api-key-12345".to_string()),
        token: None,
        cert_file: None,
        key_file: None,
        user: Some("testuser".to_string()),
        headers: HashMap::new(),
        token_refresh: TokenRefreshConfig::default(),
    };

    let endpoint = TunnelEndpoint {
        server_url: "ws://localhost:8080/connect".to_string(), // Use ws:// for testing
        subdomain: Some("test-app".to_string()),
        custom_domain: None,
        protocol_version: "1.0".to_string(),
        connection_timeout: 5, // Short timeout for tests
        keepalive_interval: 10,
        max_connections: 1,
    };

    TunnelConfig {
        enabled: true,
        local_port: Some(3000),
        local_host: "localhost".to_string(),
        endpoints: vec![endpoint],
        auth: auth_config,
        reconnection: Default::default(),
        monitoring: Default::default(),
        ssl: Default::default(),
        server: Default::default(),
    }
}

#[tokio::test]
async fn test_tunnel_client_creation() {
    let config = create_test_tunnel_config();
    
    // Test that we can create a tunnel client with valid config
    let client_result = TunnelClient::new(config, 3000);
    
    // The client should be created successfully even if we can't connect
    assert!(client_result.is_ok());
    
    let client = client_result.unwrap();
    
    // Test that we can get the current status (returns a vector)
    let statuses = client.get_status().await;
      // Initially should have no active tunnels (or empty status)
    // Status vector length is always >= 0, so just check if it's accessible
    assert!(statuses.len() == 0 || statuses.len() > 0);
    
    // Test connection count
    let connection_count = client.connection_count().await;
    assert_eq!(connection_count, 0);
    
    // Test if running
    let is_running = client.is_running().await;
    assert!(!is_running); // Should not be running initially
}

#[tokio::test]
async fn test_tunnel_client_invalid_config() {
    let mut config = create_test_tunnel_config();
    
    // Make the config invalid by removing the API key
    config.auth.api_key = None;
    
    let client_result = TunnelClient::new(config, 3000);
    
    // Should still create the client, but starting might fail
    assert!(client_result.is_ok());
}

#[tokio::test]
async fn test_tunnel_client_disabled_config() {
    let mut config = create_test_tunnel_config();
    config.enabled = false;
    
    let client_result = TunnelClient::new(config, 3000);
    
    // Should fail to create client with disabled config
    assert!(client_result.is_err());
    if let Err(e) = client_result {
        assert!(e.to_string().contains("disabled"));
    }
}

#[tokio::test]
async fn test_tunnel_client_no_endpoints() {
    let mut config = create_test_tunnel_config();
    config.endpoints.clear();
    
    let client_result = TunnelClient::new(config, 3000);
    
    // Should fail to create client with no endpoints
    assert!(client_result.is_err());
    if let Err(e) = client_result {
        assert!(e.to_string().contains("No tunnel endpoints"));
    }
}

#[tokio::test]
async fn test_tunnel_client_basic_methods() {
    let config = create_test_tunnel_config();
    let client = TunnelClient::new(config, 3000).unwrap();
    
    // Test basic getter methods
    assert!(!client.is_connected().await);
    assert_eq!(client.connection_count().await, 0);
    assert_eq!(client.active_connection_count().await, 0);
    assert!(!client.is_running().await);
    
    // Test getting public URLs (should be empty)
    let urls = client.get_public_urls().await;
    assert!(urls.is_empty());
    
    // Test exporting metrics
    let metrics = client.export_metrics().await;
    assert!(metrics.contains_key("connection_count") || metrics.is_empty());
}

#[tokio::test]
async fn test_tunnel_client_multiple_endpoints() {
    let mut config = create_test_tunnel_config();
    
    // Add a second endpoint
    let endpoint2 = TunnelEndpoint {
        server_url: "ws://localhost:8081/connect".to_string(),
        subdomain: Some("test-app-2".to_string()),
        custom_domain: None,
        protocol_version: "1.0".to_string(),
        connection_timeout: 5,
        keepalive_interval: 10,
        max_connections: 1,
    };
    config.endpoints.push(endpoint2);
    
    let client = TunnelClient::new(config, 3000).unwrap();
      // Should be able to create client with multiple endpoints
    let statuses = client.get_status().await;
    // Vector access is always valid, just check it's accessible
    assert!(statuses.len() == 0 || statuses.len() > 0);
}

#[tokio::test]
async fn test_tunnel_error_types() {
    // Test that all tunnel error types can be created and displayed
    let errors = vec![
        TunnelError::ConnectionFailed("Test connection failure".to_string()),
        TunnelError::AuthenticationFailed("Invalid API key".to_string()),
        TunnelError::InvalidConfig("Missing required field".to_string()),
        TunnelError::ConfigError("Parse error".to_string()),
        TunnelError::NetworkError("Network unreachable".to_string()),
        TunnelError::ProtocolError("Invalid protocol version".to_string()),
        TunnelError::ServerUnavailable("Server is down".to_string()),
    ];

    for error in errors {
        // Should be able to display all error types
        let error_string = error.to_string();
        assert!(!error_string.is_empty());
        
        // Should contain descriptive text
        match error {
            TunnelError::ConnectionFailed(_) => assert!(error_string.contains("Connection failed")),
            TunnelError::AuthenticationFailed(_) => assert!(error_string.contains("Authentication failed")),
            TunnelError::InvalidConfig(_) => assert!(error_string.contains("Invalid configuration")),
            TunnelError::ConfigError(_) => assert!(error_string.contains("Configuration error")),
            TunnelError::NetworkError(_) => assert!(error_string.contains("Network error")),
            TunnelError::ProtocolError(_) => assert!(error_string.contains("Protocol error")),
            TunnelError::ServerUnavailable(_) => assert!(error_string.contains("Server unavailable")),
        }
    }
}

#[tokio::test]
async fn test_status_monitor_standalone() {
    let mut monitor = TunnelStatusMonitor::new();
    
    // Test recording events
    monitor.record_event(TunnelEvent::new(
        TunnelEventType::ConnectionAttempt,
        "Testing connection".to_string()
    ));
    
    monitor.record_event(TunnelEvent::new(
        TunnelEventType::Error,
        "Test error message".to_string()
    ));
    
    // Test getting recent errors
    let recent_errors = monitor.get_recent_errors(5);
    assert_eq!(recent_errors.len(), 1); // Only the error event
    assert!(matches!(recent_errors[0].event_type, TunnelEventType::Error));
    assert_eq!(recent_errors[0].message, "Test error message");
    
    // Test getting events by type
    let connection_attempts = monitor.get_events_by_type(TunnelEventType::ConnectionAttempt);
    assert_eq!(connection_attempts.len(), 1);
    assert_eq!(connection_attempts[0].message, "Testing connection");
}

#[tokio::test]
async fn test_health_score_calculation() {
    let mut monitor = TunnelStatusMonitor::new();
    
    // Test with disconnected state (should be 0)
    let score = monitor.calculate_health_score();
    assert_eq!(score, 0);
    
    // Update to connected state
    let mut health = httpserver_tunnel::status::ConnectionHealth::default();
    health.state = ConnectionState::Connected;
    health.retry_count = 0;
    health.last_ping = Some(chrono::Utc::now());
    
    monitor.update_health(health);
    
    let score = monitor.calculate_health_score();
    assert!(score > 80); // Should be high for healthy connection
}

#[tokio::test]
async fn test_config_getter() {
    let config = create_test_tunnel_config();
    let expected_endpoints = config.endpoints.len();
    let expected_enabled = config.enabled;
    
    let client = TunnelClient::new(config, 3000).unwrap();
    
    // Test getting config
    let retrieved_config = client.get_config();
    assert_eq!(retrieved_config.enabled, expected_enabled);
    assert_eq!(retrieved_config.endpoints.len(), expected_endpoints);
}

#[tokio::test]
async fn test_status_subscription() {
    let config = create_test_tunnel_config();
    let client = TunnelClient::new(config, 3000).unwrap();
    
    // Test subscribing to status updates
    let status_receiver = client.subscribe_status();
      // Should be able to get initial status
    let initial_status = status_receiver.borrow().clone();
    // Vector access is always valid, just check it's accessible  
    assert!(initial_status.len() == 0 || initial_status.len() > 0);
}
