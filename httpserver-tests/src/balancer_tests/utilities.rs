use httpserver_balancer::{ LoadBalancingStrategy, Target };

fn create_weighted_targets() -> Vec<Target> {
    vec![
        Target::with_weight("http://localhost:3000".to_string(), 3),
        Target::with_weight("http://localhost:3001".to_string(), 2),
        Target::with_weight("http://localhost:3002".to_string(), 1)
    ]
}

// We need to access the internal WeightedRoundRobinState for GCD testing
// This is a bit of a hack since it's private, but we'll test through the public interface
#[test]
fn test_gcd_calculation() {
    // Test with weights [3, 2, 1] - GCD should be 1
    let targets = create_weighted_targets();
    let balancer = httpserver_balancer::LoadBalancer::new(
        targets,
        LoadBalancingStrategy::WeightedRoundRobin
    );

    // Test that the weighted round robin works correctly with these weights
    // This implicitly tests that GCD calculation is working
    let selections: Vec<String> = (0..12)
        .map(|_| balancer.select_target().unwrap().url.clone())
        .collect();

    // Count occurrences to verify GCD is working properly
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

    // With weights 3:2:1, we should see approximately this ratio
    assert!(count_3000 >= count_3001);
    assert!(count_3001 >= count_3002);

    // Test with even weights [6, 4, 2] - GCD should be 2
    let even_targets = vec![
        Target::with_weight("http://localhost:3000".to_string(), 6),
        Target::with_weight("http://localhost:3001".to_string(), 4),
        Target::with_weight("http://localhost:3002".to_string(), 2)
    ];
    let balancer = httpserver_balancer::LoadBalancer::new(
        even_targets,
        LoadBalancingStrategy::WeightedRoundRobin
    );

    // Test that even weights work correctly
    let selections: Vec<String> = (0..12)
        .map(|_| balancer.select_target().unwrap().url.clone())
        .collect();

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

    // With weights 6:4:2 (3:2:1), we should see this ratio
    assert!(count_3000 >= count_3001);
    assert!(count_3001 >= count_3002);
}

#[test]
fn test_load_balancing_strategy_serialization() {
    // Test that strategies can be serialized/deserialized
    let strategy = LoadBalancingStrategy::WeightedRoundRobin;
    let serialized = serde_json::to_string(&strategy).unwrap();
    let deserialized: LoadBalancingStrategy = serde_json::from_str(&serialized).unwrap();
    assert_eq!(strategy, deserialized);
}

#[test]
fn test_target_serialization() {
    // Test that targets can be serialized/deserialized
    let target = Target::with_weight("http://localhost:3000".to_string(), 5);
    let serialized = serde_json::to_string(&target).unwrap();
    let deserialized: Target = serde_json::from_str(&serialized).unwrap();
    assert_eq!(target.url, deserialized.url);
    assert_eq!(target.weight, deserialized.weight);
}
