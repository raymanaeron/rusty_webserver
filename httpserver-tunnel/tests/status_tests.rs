//! Status Monitoring Module Tests
//! Tests for health checks, metrics collection, and JSON export functionality

use httpserver_tunnel::status::{
    ConnectionHealth, TunnelMetrics, TunnelStatusMonitor, 
    ConfigSummary, TunnelEvent, TunnelEventType
};
use httpserver_tunnel::connection::ConnectionState;
use std::time::Duration;
use chrono::Utc;

#[tokio::test]
async fn test_tunnel_status_monitor_creation() {
    let monitor = TunnelStatusMonitor::new();
    
    // Create a config summary for testing
    let config_summary = ConfigSummary {
        server_url: "wss://tunnel.example.com".to_string(),
        local_port: 3000,
        subdomain: Some("test-app".to_string()),
        auth_method: "api_key".to_string(),
        auto_reconnect: true,
        ssl_verify: true,
        protocol_version: "1.0".to_string(),
    };

    let status = monitor.get_status(None, None, config_summary);
    
    // Initial state should be disconnected
    assert_eq!(status.state, ConnectionHealth::default().state);
    assert_eq!(status.public_url, None);
    assert_eq!(status.tunnel_id, None);
}

#[tokio::test]
async fn test_connection_health_default() {
    let health = ConnectionHealth::default();
    
    assert_eq!(health.state, ConnectionState::Disconnected);
    assert_eq!(health.uptime, Duration::default());
    assert_eq!(health.retry_count, 0);
    assert_eq!(health.last_error, None);
    assert_eq!(health.health_score, 0);
    assert_eq!(health.last_ping, None);
    assert_eq!(health.avg_ping_latency, None);
}

#[tokio::test]
async fn test_tunnel_metrics_creation() {
    let metrics = TunnelMetrics::new();
    
    assert_eq!(metrics.total_connections, 0);
    assert_eq!(metrics.successful_connections, 0);
    assert_eq!(metrics.failed_connections, 0);
    assert_eq!(metrics.bytes_transferred, 0);
    assert_eq!(metrics.http_requests, 0);
    assert_eq!(metrics.http_responses, 0);
    assert_eq!(metrics.avg_response_time, None);
    assert_eq!(metrics.connected_clients, 0);
    assert_eq!(metrics.server_uptime, Duration::default());
}

#[tokio::test]
async fn test_metrics_record_connection_success() {
    let mut metrics = TunnelMetrics::new();
    
    metrics.record_connection_success();
    metrics.record_connection_success();
    
    assert_eq!(metrics.total_connections, 2);
    assert_eq!(metrics.successful_connections, 2);
    assert_eq!(metrics.failed_connections, 0);
    assert_eq!(metrics.success_rate(), 1.0);
}

#[tokio::test]
async fn test_metrics_record_connection_failure() {
    let mut metrics = TunnelMetrics::new();
    
    metrics.record_connection_failure("timeout");
    metrics.record_connection_failure("network_error");
    
    assert_eq!(metrics.total_connections, 2);
    assert_eq!(metrics.successful_connections, 0);
    assert_eq!(metrics.failed_connections, 2);
    assert_eq!(metrics.success_rate(), 0.0);
    
    // Check error counts
    assert_eq!(metrics.error_counts.get("timeout"), Some(&1));
    assert_eq!(metrics.error_counts.get("network_error"), Some(&1));
}

#[tokio::test]
async fn test_metrics_success_rate() {
    let mut metrics = TunnelMetrics::new();
    
    // Test with no connections
    assert_eq!(metrics.success_rate(), 0.0);
    
    // Test with mixed success/failure
    metrics.record_connection_success();
    metrics.record_connection_success();
    metrics.record_connection_failure("error");
    
    // 2 successful out of 3 total = 0.6667
    let success_rate = metrics.success_rate();
    assert!((success_rate - 0.6667).abs() < 0.001);
}

#[tokio::test]
async fn test_metrics_http_operations() {
    let mut metrics = TunnelMetrics::new();
    
    metrics.record_http_request();
    metrics.record_http_request();
    metrics.record_http_response(Duration::from_millis(100));
    metrics.record_http_response(Duration::from_millis(200));
    
    assert_eq!(metrics.http_requests, 2);
    assert_eq!(metrics.http_responses, 2);
    
    // Average response time should be 150ms
    let avg_response = metrics.avg_response_time.unwrap();
    assert!((avg_response.as_millis() as f64 - 150.0).abs() < 1.0);
}

#[tokio::test]
async fn test_metrics_ping_latency() {
    let mut metrics = TunnelMetrics::new();
    
    // Initially no latency data
    assert!(metrics.avg_latency().is_none());
    
    // Record some ping latencies
    metrics.record_ping_latency(Duration::from_millis(50));
    metrics.record_ping_latency(Duration::from_millis(100));
    metrics.record_ping_latency(Duration::from_millis(150));
    
    let avg_latency = metrics.avg_latency().unwrap();
    assert!((avg_latency.as_millis() as f64 - 100.0).abs() < 1.0);
}

#[tokio::test]
async fn test_tunnel_event_creation() {
    let event = TunnelEvent::new(
        TunnelEventType::ConnectionSuccess, 
        "Connection established successfully".to_string()
    );
    
    assert!(matches!(event.event_type, TunnelEventType::ConnectionSuccess));
    assert_eq!(event.message, "Connection established successfully");
    assert!(event.data.is_empty());
    
    // Timestamp should be recent (within last minute)
    let now = Utc::now();
    let time_diff = now.signed_duration_since(event.timestamp);
    assert!(time_diff.num_seconds() < 60);
}

#[tokio::test]
async fn test_tunnel_event_with_data() {
    let mut data = std::collections::HashMap::new();
    data.insert("connection_id".to_string(), serde_json::Value::String("conn-123".to_string()));
    data.insert("latency_ms".to_string(), serde_json::Value::Number(serde_json::Number::from(45)));
    
    let event = TunnelEvent::with_data(
        TunnelEventType::Ping,
        "Ping sent to server".to_string(),
        data.clone()
    );
    
    assert!(matches!(event.event_type, TunnelEventType::Ping));
    assert_eq!(event.message, "Ping sent to server");
    assert_eq!(event.data.len(), 2);
    assert_eq!(event.data.get("connection_id"), Some(&serde_json::Value::String("conn-123".to_string())));
}

#[tokio::test]
async fn test_status_monitor_event_recording() {
    let mut monitor = TunnelStatusMonitor::new();
    
    let event1 = TunnelEvent::new(TunnelEventType::ConnectionAttempt, "Attempting connection".to_string());
    let event2 = TunnelEvent::new(TunnelEventType::ConnectionSuccess, "Connected successfully".to_string());
    
    monitor.record_event(event1);
    monitor.record_event(event2);
    
    let config_summary = ConfigSummary {
        server_url: "wss://test.com".to_string(),
        local_port: 3000,
        subdomain: None,
        auth_method: "api_key".to_string(),
        auto_reconnect: true,
        ssl_verify: true,
        protocol_version: "1.0".to_string(),
    };
    
    let status = monitor.get_status(None, None, config_summary);
    
    // Should have 2 events (limited to last 50, but we only have 2)
    assert_eq!(status.recent_events.len(), 2);
    
    // Events should be in reverse order (most recent first)
    assert!(matches!(status.recent_events[0].event_type, TunnelEventType::ConnectionSuccess));
    assert!(matches!(status.recent_events[1].event_type, TunnelEventType::ConnectionAttempt));
}

#[tokio::test]
async fn test_status_monitor_health_updates() {
    let mut monitor = TunnelStatusMonitor::new();
    
    let mut health = ConnectionHealth::default();
    health.state = ConnectionState::Connected;
    health.health_score = 95;
    health.uptime = Duration::from_secs(3600); // 1 hour
    
    monitor.update_health(health);
    
    let config_summary = ConfigSummary {
        server_url: "wss://test.com".to_string(),
        local_port: 8080,
        subdomain: Some("test".to_string()),
        auth_method: "token".to_string(),
        auto_reconnect: false,
        ssl_verify: true,
        protocol_version: "1.1".to_string(),
    };
    
    let status = monitor.get_status(
        Some("https://test.tunnel.com".to_string()),
        Some("tunnel-abc123".to_string()),
        config_summary
    );
    
    assert_eq!(status.state, ConnectionState::Connected);
    assert_eq!(status.public_url, Some("https://test.tunnel.com".to_string()));
    assert_eq!(status.tunnel_id, Some("tunnel-abc123".to_string()));
    assert_eq!(status.health.health_score, 95);
    assert_eq!(status.health.uptime, Duration::from_secs(3600));
}

#[tokio::test]
async fn test_status_monitor_metrics_export() {
    let mut monitor = TunnelStatusMonitor::new();
    
    let mut metrics = TunnelMetrics::new();
    metrics.record_connection_success();
    metrics.record_http_request();
    
    monitor.update_metrics(metrics);
    
    let exported = monitor.export_metrics();
    
    // Check that required fields are present
    assert!(exported.contains_key("total_connections"));
    assert!(exported.contains_key("successful_connections"));
    assert!(exported.contains_key("http_requests"));
    assert!(exported.contains_key("health_score"));
    assert!(exported.contains_key("connection_state"));
    
    // Verify values
    assert_eq!(exported["total_connections"], serde_json::Value::Number(1.into()));
    assert_eq!(exported["successful_connections"], serde_json::Value::Number(1.into()));
    assert_eq!(exported["http_requests"], serde_json::Value::Number(1.into()));
}
