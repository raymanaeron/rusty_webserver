use httpserver_balancer::{ LoadBalancer, LoadBalancingStrategy, Target };

fn create_test_targets() -> Vec<Target> {
    vec![
        Target::new("http://localhost:3000".to_string()),
        Target::new("http://localhost:3001".to_string()),
        Target::new("http://localhost:3002".to_string())
    ]
}

#[test]
fn test_no_healthy_targets() {
    let mut targets = create_test_targets();
    for target in &mut targets {
        target.healthy = false;
    }

    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::RoundRobin);
    assert!(balancer.select_target().is_none());
    assert_eq!(balancer.healthy_targets_count(), 0);
}

#[test]
fn test_target_health_management() {
    let targets = create_test_targets();
    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::RoundRobin);

    assert_eq!(balancer.healthy_targets_count(), 3);

    // Mark one target as unhealthy
    balancer.set_target_health("http://localhost:3001", false);
    assert_eq!(balancer.healthy_targets_count(), 2);

    // Verify unhealthy target is not selected
    let selections: Vec<String> = (0..6)
        .map(|_| balancer.select_target().unwrap().url.clone())
        .collect();

    for selection in selections {
        assert_ne!(selection, "http://localhost:3001");
    }

    // Mark target as healthy again
    balancer.set_target_health("http://localhost:3001", true);
    assert_eq!(balancer.healthy_targets_count(), 3);
}

#[test]
fn test_empty_targets() {
    let balancer = LoadBalancer::new(vec![], LoadBalancingStrategy::RoundRobin);
    assert!(balancer.select_target().is_none());
    assert_eq!(balancer.healthy_targets_count(), 0);
}

#[test]
fn test_single_target() {
    let targets = vec![Target::new("http://localhost:3000".to_string())];
    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::RoundRobin);

    // All selections should return the same target
    for _ in 0..5 {
        let target = balancer.select_target().unwrap();
        assert_eq!(target.url, "http://localhost:3000");
    }
}
