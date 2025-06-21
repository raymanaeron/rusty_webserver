use std::sync::Arc;
use httpserver_proxy::{ HealthCheckIntegration, HealthSummary };
use httpserver_config::{ LoadBalancingStrategy, WebSocketHealthConfig };
use httpserver_balancer::{ LoadBalancer, Target };

#[tokio::test]
async fn test_health_check_integration() {
    // Create targets for testing
    let targets = vec![
        Target::new("http://localhost:5000".to_string()),
        Target::new("http://localhost:5001".to_string()),
        Target::new("http://localhost:5002".to_string())
    ];

    // Create load balancer
    let load_balancer = Arc::new(LoadBalancer::new(targets, LoadBalancingStrategy::RoundRobin));

    // Create health check integration
    let health_integration = HealthCheckIntegration::new(load_balancer.clone());

    // Get initial health summary
    let initial_summary = health_integration.get_health_summary();
    assert_eq!(initial_summary.total_targets, 3);
    assert_eq!(initial_summary.healthy_targets, 3); // All targets start healthy
    assert_eq!(initial_summary.unhealthy_targets, 0);
    assert!(!initial_summary.monitoring_enabled);

    // Test health summary display
    let summary_str = format!("{}", initial_summary);
    assert!(summary_str.contains("3/3 healthy targets"));
    assert!(summary_str.contains("monitoring: disabled"));
}

#[tokio::test]
async fn test_health_status_updates() {
    // Create targets for testing
    let targets = vec![
        Target::new("http://localhost:5000".to_string()),
        Target::new("http://localhost:5001".to_string())
    ];

    // Create load balancer
    let load_balancer = Arc::new(LoadBalancer::new(targets, LoadBalancingStrategy::RoundRobin));

    // Test manual health status updates
    assert_eq!(load_balancer.healthy_targets_count(), 2);

    // Mark one target as unhealthy
    load_balancer.set_target_health("http://localhost:5000", false);
    assert_eq!(load_balancer.healthy_targets_count(), 1);

    // Mark it as healthy again
    load_balancer.set_target_health("http://localhost:5000", true);
    assert_eq!(load_balancer.healthy_targets_count(), 2);

    // Test with health integration
    let health_integration = HealthCheckIntegration::new(load_balancer.clone());
    let summary = health_integration.get_health_summary();
    assert_eq!(summary.total_targets, 2);
    assert_eq!(summary.healthy_targets, 2);
}

#[tokio::test]
async fn test_websocket_health_monitoring_setup() {
    // Create targets for testing
    let targets = vec![
        Target::new("http://localhost:8080".to_string()),
        Target::new("http://localhost:8081".to_string())
    ];

    // Create load balancer
    let load_balancer = Arc::new(
        LoadBalancer::new(targets, LoadBalancingStrategy::LeastConnections)
    );

    // Create health check integration
    let mut health_integration = HealthCheckIntegration::new(load_balancer.clone());

    // Create WebSocket health config
    let health_config = WebSocketHealthConfig {
        interval: 10, // 10 seconds
        timeout: 2, // 2 seconds
        path: "/health".to_string(),
        ping_message: "ping".to_string(),
    };

    // Start WebSocket health monitoring (will fail to connect but tests the setup)
    let result = health_integration.start_websocket_health_monitoring(health_config).await;
    assert!(result.is_ok());

    // Check that monitoring is now enabled
    let summary = health_integration.get_health_summary();
    assert!(summary.monitoring_enabled);
}

#[tokio::test]
async fn test_empty_targets_health_monitoring() {
    // Create load balancer with no targets
    let load_balancer = Arc::new(LoadBalancer::new(vec![], LoadBalancingStrategy::RoundRobin));
    // Create health check integration
    let mut health_integration = HealthCheckIntegration::new(load_balancer.clone());

    // Try to start health monitoring with no targets
    let health_config = WebSocketHealthConfig {
        interval: 5,
        timeout: 1,
        path: "/ping".to_string(),
        ping_message: "test".to_string(),
    };

    let result = health_integration.start_websocket_health_monitoring(health_config).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("No targets available"));

    // Check summary for empty targets
    let summary = health_integration.get_health_summary();
    assert_eq!(summary.total_targets, 0);
    assert_eq!(summary.healthy_targets, 0);
    assert_eq!(summary.unhealthy_targets, 0);
    assert!(!summary.monitoring_enabled);
}

#[test]
fn test_health_summary_display() {
    // Test different health summary states
    let summary1 = HealthSummary {
        total_targets: 5,
        healthy_targets: 5,
        unhealthy_targets: 0,
        monitoring_enabled: true,
    };

    let display1 = format!("{}", summary1);
    assert!(display1.contains("5/5 healthy targets"));
    assert!(display1.contains("monitoring: enabled"));

    let summary2 = HealthSummary {
        total_targets: 3,
        healthy_targets: 1,
        unhealthy_targets: 2,
        monitoring_enabled: false,
    };

    let display2 = format!("{}", summary2);
    assert!(display2.contains("1/3 healthy targets"));
    assert!(display2.contains("monitoring: disabled"));
}

#[tokio::test]
async fn test_health_callback_mechanism() {
    // Create targets for testing
    let targets = vec![
        Target::new("http://localhost:9000".to_string()),
        Target::new("http://localhost:9001".to_string())
    ];

    // Create load balancer
    let load_balancer = Arc::new(
        LoadBalancer::new(targets, LoadBalancingStrategy::WeightedRoundRobin)
    );

    // Test that health callback updates load balancer
    assert_eq!(load_balancer.healthy_targets_count(), 2);

    // Simulate health callback marking a target unhealthy
    load_balancer.set_target_health("http://localhost:9000", false);
    assert_eq!(load_balancer.healthy_targets_count(), 1);

    // Simulate health callback marking target healthy again
    load_balancer.set_target_health("http://localhost:9000", true);
    assert_eq!(load_balancer.healthy_targets_count(), 2);

    // Verify that target selection is affected by health status
    load_balancer.set_target_health("http://localhost:9001", false);

    // Only one target should be available now
    let available_target = load_balancer.select_target();
    assert!(available_target.is_some());
    assert_eq!(available_target.unwrap().url, "http://localhost:9000");

    // Mark all targets unhealthy
    load_balancer.set_target_health("http://localhost:9000", false);
    assert!(load_balancer.select_target().is_none());
}
