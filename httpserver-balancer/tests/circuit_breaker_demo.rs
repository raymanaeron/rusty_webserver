// Circuit breaker functionality demonstration test
use httpserver_balancer::{LoadBalancer, LoadBalancingStrategy, Target, CircuitBreakerConfig};

#[test]
fn test_circuit_breaker_demo() {
    // Create a load balancer with circuit breaker
    let targets = vec![
        Target::new("http://localhost:3001".to_string()),
        Target::new("http://localhost:3002".to_string()),
    ];
    
    let load_balancer = LoadBalancer::new(targets, LoadBalancingStrategy::RoundRobin);
      // Configure circuit breaker for a target
    let circuit_config = CircuitBreakerConfig {
        enabled: true,
        failure_threshold: 3,
        failure_window: 30,
        open_timeout: 15,
        test_requests: 2,
        min_requests: 3, // Reduced to allow circuit to open after 3 failures
    };
    
    load_balancer.initialize_circuit_breaker("http://localhost:3001", circuit_config);
    
    println!("Circuit breaker initialized for http://localhost:3001");
    
    // Initially, requests should be allowed
    assert!(load_balancer.allow_request("http://localhost:3001"));
    
    // Simulate some failures
    for i in 1..=5 {
        load_balancer.record_failure("http://localhost:3001");
        println!("Recorded failure #{} for target", i);
        
        let allowed = load_balancer.allow_request("http://localhost:3001");
        println!("Request allowed: {}", allowed);
        
        if let Some(stats) = load_balancer.get_circuit_breaker_stats("http://localhost:3001") {
            println!("Circuit state: {:?}, failures: {}", stats.state, stats.failure_count);
            
            // After 3 failures (the threshold), circuit should be open
            if i >= 3 {
                // Circuit should be open, so requests should not be allowed
                assert!(!allowed, "Circuit should be open after {} failures", i);
            }
        }
        println!("---");
    }
    
    // Test target selection with circuit breaker
    // Should select the healthy target (localhost:3002) since localhost:3001 circuit is open
    if let Some(target) = load_balancer.select_target_with_circuit_breaker() {
        println!("Selected target: {}", target.url);
        // Should select the healthy target, not the one with open circuit
        assert_eq!(target.url, "http://localhost:3002");
    } else {
        panic!("Should have at least one healthy target available");
    }
    
    // Verify circuit breaker stats
    if let Some(stats) = load_balancer.get_circuit_breaker_stats("http://localhost:3001") {
        assert_eq!(stats.failure_count, 5);
        assert!(matches!(stats.state, httpserver_balancer::CircuitState::Open));
        println!("Final circuit state verified: Open with 5 failures");
    } else {
        panic!("Should have circuit breaker stats for the target");
    }
}

#[test]
fn test_circuit_breaker_recovery() {
    // Test circuit breaker recovery functionality
    let targets = vec![Target::new("http://localhost:3001".to_string())];
    let load_balancer = LoadBalancer::new(targets, LoadBalancingStrategy::RoundRobin);
    
    let circuit_config = CircuitBreakerConfig {
        enabled: true,
        failure_threshold: 2,
        failure_window: 60,
        open_timeout: 1, // Short timeout for testing
        test_requests: 1,
        min_requests: 2,
    };
    
    load_balancer.initialize_circuit_breaker("http://localhost:3001", circuit_config);
    
    // Trigger circuit breaker
    load_balancer.record_failure("http://localhost:3001");
    load_balancer.record_failure("http://localhost:3001");
    
    // Circuit should be open
    assert!(!load_balancer.allow_request("http://localhost:3001"));
    
    // Wait for timeout (in real scenario, would need actual time delay)
    // For testing, we simulate recovery by recording success
    std::thread::sleep(std::time::Duration::from_millis(1100)); // Wait slightly longer than timeout
    
    // Circuit should allow test request in half-open state
    let allowed = load_balancer.allow_request("http://localhost:3001");
    println!("Request allowed after timeout: {}", allowed);
    
    // Record a success to close the circuit
    load_balancer.record_success("http://localhost:3001");
    
    // Circuit should now be closed and allow requests
    assert!(load_balancer.allow_request("http://localhost:3001"));
    
    if let Some(stats) = load_balancer.get_circuit_breaker_stats("http://localhost:3001") {
        println!("Circuit recovered to state: {:?}", stats.state);
    }
}
