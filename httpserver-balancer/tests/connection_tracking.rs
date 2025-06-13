use httpserver_balancer::{LoadBalancer, LoadBalancingStrategy, Target};

fn create_test_targets() -> Vec<Target> {
    vec![
        Target::new("http://localhost:3000".to_string()),
        Target::new("http://localhost:3001".to_string()),
        Target::new("http://localhost:3002".to_string()),
    ]
}

#[test]
fn test_connection_tracking() {
    let targets = create_test_targets();
    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::LeastConnections);

    let target_url = "http://localhost:3000";

    // Test connection increment/decrement
    assert_eq!(balancer.get_connection_count(target_url), 0);

    balancer.start_request(target_url);
    assert_eq!(balancer.get_connection_count(target_url), 1);

    balancer.start_request(target_url);
    assert_eq!(balancer.get_connection_count(target_url), 2);

    balancer.end_request(target_url);
    assert_eq!(balancer.get_connection_count(target_url), 1);

    balancer.end_request(target_url);
    assert_eq!(balancer.get_connection_count(target_url), 0);

    // Test that decrementing below 0 doesn't go negative
    balancer.end_request(target_url);
    assert_eq!(balancer.get_connection_count(target_url), 0);
}
