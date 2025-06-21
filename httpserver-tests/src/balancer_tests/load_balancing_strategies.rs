use httpserver_balancer::{ LoadBalancer, LoadBalancingStrategy, Target };

fn create_test_targets() -> Vec<Target> {
    vec![
        Target::new("http://localhost:3000".to_string()),
        Target::new("http://localhost:3001".to_string()),
        Target::new("http://localhost:3002".to_string())
    ]
}

fn create_weighted_targets() -> Vec<Target> {
    vec![
        Target::with_weight("http://localhost:3000".to_string(), 3),
        Target::with_weight("http://localhost:3001".to_string(), 2),
        Target::with_weight("http://localhost:3002".to_string(), 1)
    ]
}

#[test]
fn test_round_robin_strategy() {
    let targets = create_test_targets();
    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::RoundRobin);

    // Test round-robin distribution
    let selections: Vec<String> = (0..6)
        .map(|_| balancer.select_target().unwrap().url.clone())
        .collect();

    assert_eq!(selections[0], "http://localhost:3000");
    assert_eq!(selections[1], "http://localhost:3001");
    assert_eq!(selections[2], "http://localhost:3002");
    assert_eq!(selections[3], "http://localhost:3000"); // Wraps around
    assert_eq!(selections[4], "http://localhost:3001");
    assert_eq!(selections[5], "http://localhost:3002");
}

#[test]
fn test_weighted_round_robin_strategy() {
    let targets = create_weighted_targets();
    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::WeightedRoundRobin);

    // Collect many selections to test weight distribution
    let selections: Vec<String> = (0..30)
        .map(|_| balancer.select_target().unwrap().url.clone())
        .collect();

    // Count occurrences
    let count_3000 = selections
        .iter()
        .filter(|&url| url == "http://localhost:3000")
        .count();
    let count_3001 = selections
        .iter()
        .filter(|&url| url == "http://localhost:3001")
        .count();
    let count_3002 = selections
        .iter()
        .filter(|&url| url == "http://localhost:3002")
        .count();

    // Verify weight ratios (approximately 3:2:1)
    // With weight 3:2:1, we expect roughly 50% : 33% : 17%
    assert!(count_3000 > count_3001, "Weight 3 target should get more requests than weight 2");
    assert!(count_3001 > count_3002, "Weight 2 target should get more requests than weight 1");
}

#[test]
fn test_random_strategy() {
    let targets = create_test_targets();
    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::Random);

    // Test that random selection works and returns valid targets
    let selections: Vec<String> = (0..10)
        .map(|_| balancer.select_target().unwrap().url.clone())
        .collect();

    // Verify all selections are valid
    for selection in selections {
        assert!(selection.starts_with("http://localhost:300"));
    }
}

#[test]
fn test_least_connections_strategy() {
    let targets = create_test_targets();
    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::LeastConnections);

    // Initially, all targets have 0 connections, so first target should be selected
    let target1 = balancer.select_target().unwrap();
    balancer.start_request(&target1.url);

    // Next selection should prefer a different target (with 0 connections)
    let target2 = balancer.select_target().unwrap();
    assert_ne!(target1.url, target2.url);
    balancer.start_request(&target2.url);

    // Add more connections to target1
    balancer.start_request(&target1.url);
    balancer.start_request(&target1.url);

    // Now target2 should have fewer connections and be preferred
    let target3 = balancer.select_target().unwrap();
    // Should prefer target2 or a target with fewer connections
    assert!(
        balancer.get_connection_count(&target3.url) <= balancer.get_connection_count(&target1.url)
    );

    // Test connection cleanup
    balancer.end_request(&target1.url);
    assert_eq!(balancer.get_connection_count(&target1.url), 2);
}
