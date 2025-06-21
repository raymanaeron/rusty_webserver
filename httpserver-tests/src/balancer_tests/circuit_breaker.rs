// Circuit breaker pattern tests
use httpserver_balancer::{
    LoadBalancer,
    LoadBalancingStrategy,
    Target,
    CircuitBreaker,
    CircuitState,
    CircuitBreakerConfig,
};
use std::time::{ Duration, Instant };

fn create_test_targets() -> Vec<Target> {
    vec![
        Target::new("http://localhost:3000".to_string()),
        Target::new("http://localhost:3001".to_string()),
        Target::new("http://localhost:3002".to_string())
    ]
}

fn create_circuit_breaker_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig {
        enabled: true,
        failure_threshold: 3,
        failure_window: 60,
        open_timeout: 5,
        test_requests: 2,
        min_requests: 2,
    }
}

#[test]
fn test_circuit_breaker_creation() {
    let config = create_circuit_breaker_config();
    let circuit_breaker = CircuitBreaker::new(config.clone());

    assert_eq!(circuit_breaker.state, CircuitState::Closed);
    assert_eq!(circuit_breaker.failure_count, 0);
    assert_eq!(circuit_breaker.success_count, 0);
    assert_eq!(circuit_breaker.total_requests, 0);
    assert!(circuit_breaker.last_failure_time.is_none());
    assert_eq!(circuit_breaker.config.failure_threshold, 3);
}

#[test]
fn test_circuit_breaker_allow_request_when_disabled() {
    let config = CircuitBreakerConfig {
        enabled: false,
        ..create_circuit_breaker_config()
    };
    let mut circuit_breaker = CircuitBreaker::new(config);

    // Should always allow requests when disabled
    assert!(circuit_breaker.allow_request());

    // Record failures - should still allow requests
    circuit_breaker.record_failure();
    circuit_breaker.record_failure();
    circuit_breaker.record_failure();
    assert!(circuit_breaker.allow_request());
}

#[test]
fn test_circuit_breaker_success_recording() {
    let config = create_circuit_breaker_config();
    let mut circuit_breaker = CircuitBreaker::new(config);

    // Record a success
    circuit_breaker.record_success();

    assert_eq!(circuit_breaker.state, CircuitState::Closed);
    assert_eq!(circuit_breaker.success_count, 1);
    assert_eq!(circuit_breaker.total_requests, 1);
    assert_eq!(circuit_breaker.failure_count, 0);
}

#[test]
fn test_circuit_breaker_failure_recording() {
    let config = create_circuit_breaker_config();
    let mut circuit_breaker = CircuitBreaker::new(config);

    // Record minimum requests first
    circuit_breaker.record_success();
    circuit_breaker.record_success();

    // Now record failures
    circuit_breaker.record_failure();
    assert_eq!(circuit_breaker.state, CircuitState::Closed);
    assert_eq!(circuit_breaker.failure_count, 1);

    circuit_breaker.record_failure();
    assert_eq!(circuit_breaker.state, CircuitState::Closed);
    assert_eq!(circuit_breaker.failure_count, 2);

    // Third failure should open the circuit
    circuit_breaker.record_failure();
    assert_eq!(circuit_breaker.state, CircuitState::Open);
    assert_eq!(circuit_breaker.failure_count, 3);
}

#[test]
fn test_circuit_breaker_state_transitions() {
    let config = create_circuit_breaker_config();
    let mut circuit_breaker = CircuitBreaker::new(config);

    // Start in closed state
    assert_eq!(circuit_breaker.state, CircuitState::Closed);
    assert!(circuit_breaker.allow_request());

    // Record minimum requests and failures to open circuit
    circuit_breaker.record_success();
    circuit_breaker.record_success();
    circuit_breaker.record_failure();
    circuit_breaker.record_failure();
    circuit_breaker.record_failure();

    // Should be open now
    assert_eq!(circuit_breaker.state, CircuitState::Open);
    assert!(!circuit_breaker.allow_request());

    // Simulate time passing (we can't actually wait 5 seconds in test)
    // We'll test the transition logic by manipulating the state change time
    circuit_breaker.state_change_time = Instant::now() - Duration::from_secs(6);

    // Next request should transition to half-open
    assert!(circuit_breaker.allow_request());
    assert_eq!(circuit_breaker.state, CircuitState::HalfOpen);

    // Record successful test requests
    circuit_breaker.record_success();
    assert_eq!(circuit_breaker.state, CircuitState::HalfOpen);

    circuit_breaker.record_success(); // Second success should close circuit
    assert_eq!(circuit_breaker.state, CircuitState::Closed);
}

#[test]
fn test_circuit_breaker_half_open_failure() {
    let config = create_circuit_breaker_config();
    let mut circuit_breaker = CircuitBreaker::new(config);

    // Open the circuit
    circuit_breaker.record_success();
    circuit_breaker.record_success();
    circuit_breaker.record_failure();
    circuit_breaker.record_failure();
    circuit_breaker.record_failure();
    assert_eq!(circuit_breaker.state, CircuitState::Open);

    // Transition to half-open
    circuit_breaker.state_change_time = Instant::now() - Duration::from_secs(6);
    assert!(circuit_breaker.allow_request());
    assert_eq!(circuit_breaker.state, CircuitState::HalfOpen);

    // Failure during half-open should reopen circuit
    circuit_breaker.record_failure();
    assert_eq!(circuit_breaker.state, CircuitState::Open);
    assert_eq!(circuit_breaker.success_count, 0);
}

#[test]
fn test_circuit_breaker_statistics() {
    let config = create_circuit_breaker_config();
    let mut circuit_breaker = CircuitBreaker::new(config);

    // Record some activity
    circuit_breaker.record_success();
    circuit_breaker.record_failure();

    let stats = circuit_breaker.get_stats();
    assert_eq!(stats.state, CircuitState::Closed);
    assert_eq!(stats.success_count, 1);
    assert_eq!(stats.failure_count, 1);
    assert_eq!(stats.total_requests, 2);
    assert!(stats.last_failure_time.is_some());
}

#[test]
fn test_load_balancer_circuit_breaker_integration() {
    let targets = create_test_targets();
    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::RoundRobin);

    // Initialize circuit breaker for a target
    let config = create_circuit_breaker_config();
    balancer.initialize_circuit_breaker("http://localhost:3000", config);

    // Should allow requests initially
    assert!(balancer.allow_request("http://localhost:3000"));

    // Record success
    balancer.record_success("http://localhost:3000");
    balancer.record_success("http://localhost:3000");

    // Record failures to open circuit
    balancer.record_failure("http://localhost:3000");
    balancer.record_failure("http://localhost:3000");
    balancer.record_failure("http://localhost:3000");

    // Circuit should be open now
    assert!(!balancer.allow_request("http://localhost:3000"));

    // Get statistics
    let stats = balancer.get_circuit_breaker_stats("http://localhost:3000");
    assert!(stats.is_some());
    assert_eq!(stats.unwrap().state, CircuitState::Open);
}

#[test]
fn test_load_balancer_circuit_breaker_target_selection() {
    let targets = create_test_targets();
    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::RoundRobin);

    // Initialize circuit breakers for all targets
    let config = create_circuit_breaker_config();
    balancer.initialize_circuit_breaker("http://localhost:3000", config.clone());
    balancer.initialize_circuit_breaker("http://localhost:3001", config.clone());
    balancer.initialize_circuit_breaker("http://localhost:3002", config);

    // All targets should be available initially
    let target = balancer.select_target_with_circuit_breaker();
    assert!(target.is_some());

    // Open circuit for first target
    balancer.record_success("http://localhost:3000");
    balancer.record_success("http://localhost:3000");
    balancer.record_failure("http://localhost:3000");
    balancer.record_failure("http://localhost:3000");
    balancer.record_failure("http://localhost:3000");

    // Should still be able to select other targets
    for _ in 0..10 {
        let target = balancer.select_target_with_circuit_breaker();
        assert!(target.is_some());
        let target_url = &target.unwrap().url;
        assert_ne!(target_url, "http://localhost:3000"); // Should not select the open circuit target
    }
}

#[test]
fn test_load_balancer_all_circuits_open() {
    let targets = create_test_targets();
    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::RoundRobin);

    // Initialize circuit breakers and open all circuits
    let config = create_circuit_breaker_config();
    for target in balancer.targets() {
        balancer.initialize_circuit_breaker(&target.url, config.clone());

        // Record minimum requests and failures to open each circuit
        balancer.record_success(&target.url);
        balancer.record_success(&target.url);
        balancer.record_failure(&target.url);
        balancer.record_failure(&target.url);
        balancer.record_failure(&target.url);
    }

    // No targets should be available
    let target = balancer.select_target_with_circuit_breaker();
    assert!(target.is_none());
}

#[test]
fn test_circuit_breaker_get_all_stats() {
    let targets = create_test_targets();
    let balancer = LoadBalancer::new(targets, LoadBalancingStrategy::RoundRobin);

    // Initialize circuit breakers
    let config = create_circuit_breaker_config();
    balancer.initialize_circuit_breaker("http://localhost:3000", config.clone());
    balancer.initialize_circuit_breaker("http://localhost:3001", config);

    // Record some activity
    balancer.record_success("http://localhost:3000");
    balancer.record_failure("http://localhost:3001");

    // Get all statistics
    let all_stats = balancer.get_all_circuit_breaker_stats();
    assert_eq!(all_stats.len(), 2);
    assert!(all_stats.contains_key("http://localhost:3000"));
    assert!(all_stats.contains_key("http://localhost:3001"));

    let stats_3000 = &all_stats["http://localhost:3000"];
    assert_eq!(stats_3000.success_count, 1);
    assert_eq!(stats_3000.failure_count, 0);

    let stats_3001 = &all_stats["http://localhost:3001"];
    assert_eq!(stats_3001.success_count, 0);
    assert_eq!(stats_3001.failure_count, 1);
}
