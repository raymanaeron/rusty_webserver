use serde::{ Deserialize, Serialize };
use std::sync::{ Arc, Mutex };
use std::collections::HashMap;
use std::hash::{ Hash, Hasher };
use std::collections::hash_map::DefaultHasher;
use axum::{ routing::get, Router, Json };
use serde_json::{ json, Value };
use tracing;
use std::time::{ Duration, Instant };

/// Health endpoint handler for load balancer service
pub async fn balancer_health() -> Json<Value> {
    Json(
        json!({
        "status": "healthy",
        "service": "httpserver-balancer",
        "message": "Load balancing service operational"
    })
    )
}

/// Create balancer service health router
pub fn create_balancer_health_router() -> Router {
    Router::new()
        .route("/balancer/health", get(balancer_health))
        .route("/balancer/status", get(balancer_health))
}

/// Load balancing strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LoadBalancingStrategy {
    /// Simple round-robin selection
    RoundRobin,
    /// Weighted round-robin selection
    WeightedRoundRobin,
    /// Random target selection
    Random,
    /// Route to target with least active connections
    LeastConnections,
}

impl std::fmt::Display for LoadBalancingStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadBalancingStrategy::RoundRobin => write!(f, "round_robin"),
            LoadBalancingStrategy::WeightedRoundRobin => write!(f, "weighted_round_robin"),
            LoadBalancingStrategy::Random => write!(f, "random"),
            LoadBalancingStrategy::LeastConnections => write!(f, "least_connections"),
        }
    }
}

impl Default for LoadBalancingStrategy {
    fn default() -> Self {
        Self::RoundRobin
    }
}

/// Target server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    /// Target URL
    pub url: String,
    /// Weight for weighted round-robin (default: 1)
    #[serde(default = "default_weight")]
    pub weight: u32,
    /// Whether this target is currently healthy
    #[serde(skip, default = "default_healthy")]
    pub healthy: bool,
}

impl Target {
    pub fn new(url: String) -> Self {
        Self {
            url,
            weight: 1,
            healthy: true,
        }
    }

    pub fn with_weight(url: String, weight: u32) -> Self {
        Self {
            url,
            weight,
            healthy: true,
        }
    }
}

fn default_weight() -> u32 {
    1
}

fn default_healthy() -> bool {
    true
}

/// Connection tracking for least-connections strategy
#[derive(Debug, Default)]
struct ConnectionTracker {
    active_connections: HashMap<String, u32>,
}

impl ConnectionTracker {
    fn new() -> Self {
        Self {
            active_connections: HashMap::new(),
        }
    }

    fn get_connections(&self, target_url: &str) -> u32 {
        self.active_connections.get(target_url).copied().unwrap_or(0)
    }

    fn increment(&mut self, target_url: &str) {
        *self.active_connections.entry(target_url.to_string()).or_insert(0) += 1;
    }

    fn decrement(&mut self, target_url: &str) {
        if let Some(count) = self.active_connections.get_mut(target_url) {
            if *count > 0 {
                *count -= 1;
            }
        }
    }
}

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed, // Normal operation
    Open, // Failures exceeded threshold, blocking requests
    HalfOpen, // Testing if service has recovered
}

/// Circuit breaker for a single target
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    pub state: CircuitState,
    pub failure_count: u32,
    pub success_count: u32,
    pub total_requests: u32,
    pub last_failure_time: Option<Instant>,
    pub state_change_time: Instant,
    pub config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            total_requests: 0,
            last_failure_time: None,
            state_change_time: Instant::now(),
            config,
        }
    }

    /// Record a successful request
    pub fn record_success(&mut self) {
        self.total_requests += 1;

        match self.state {
            CircuitState::Closed => {
                self.success_count += 1;
                // Reset failure count on success
                self.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                self.success_count += 1;

                // If we have enough successful test requests, close the circuit
                if self.success_count >= self.config.test_requests {
                    self.state = CircuitState::Closed;
                    self.state_change_time = Instant::now();
                    self.failure_count = 0;
                    self.success_count = 0;

                    tracing::info!(
                        state = "closed",
                        "Circuit breaker closed after successful test requests"
                    );
                }
            }
            CircuitState::Open => {
                // Should not happen - requests shouldn't reach open circuit
                tracing::warn!("Received success response while circuit is open");
            }
        }
    }

    /// Record a failed request
    pub fn record_failure(&mut self) {
        self.total_requests += 1;
        self.failure_count += 1;
        self.last_failure_time = Some(Instant::now());

        match self.state {
            CircuitState::Closed => {
                // Check if we should open the circuit
                if
                    self.total_requests >= self.config.min_requests &&
                    self.failure_count >= self.config.failure_threshold
                {
                    self.state = CircuitState::Open;
                    self.state_change_time = Instant::now();

                    tracing::warn!(
                        failure_count = self.failure_count,
                        threshold = self.config.failure_threshold,
                        "Circuit breaker opened due to failures"
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Failure during test - reopen circuit
                self.state = CircuitState::Open;
                self.state_change_time = Instant::now();
                self.success_count = 0;

                tracing::warn!("Circuit breaker reopened after failure during test");
            }
            CircuitState::Open => {
                // Already open, just record the failure
            }
        }
    }

    /// Check if a request should be allowed through
    pub fn allow_request(&mut self) -> bool {
        if !self.config.enabled {
            return true;
        }

        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if we should transition to half-open
                let time_since_open = self.state_change_time.elapsed();
                if time_since_open >= Duration::from_secs(self.config.open_timeout) {
                    self.state = CircuitState::HalfOpen;
                    self.state_change_time = Instant::now();
                    self.success_count = 0;

                    tracing::info!("Circuit breaker transitioned to half-open for testing");

                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                // Allow limited test requests
                self.success_count < self.config.test_requests
            }
        }
    }

    /// Get current circuit breaker statistics
    pub fn get_stats(&self) -> CircuitBreakerStats {
        CircuitBreakerStats {
            state: self.state.clone(),
            failure_count: self.failure_count,
            success_count: self.success_count,
            total_requests: self.total_requests,
            last_failure_time: self.last_failure_time,
            state_change_time: self.state_change_time,
        }
    }
}

/// Circuit breaker statistics for monitoring
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    pub state: CircuitState,
    pub failure_count: u32,
    pub success_count: u32,
    pub total_requests: u32,
    pub last_failure_time: Option<Instant>,
    pub state_change_time: Instant,
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Enable circuit breaker functionality
    #[serde(default)]
    pub enabled: bool,

    /// Failure threshold to trigger circuit open (default: 5)
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,

    /// Time window in seconds for failure counting (default: 60)
    #[serde(default = "default_failure_window")]
    pub failure_window: u64,

    /// Duration in seconds to keep circuit open (default: 30)
    #[serde(default = "default_open_timeout")]
    pub open_timeout: u64,

    /// Number of test requests in half-open state (default: 3)
    #[serde(default = "default_test_requests")]
    pub test_requests: u32,

    /// Minimum requests before circuit breaker activates (default: 10)
    #[serde(default = "default_min_requests")]
    pub min_requests: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            failure_threshold: default_failure_threshold(),
            failure_window: default_failure_window(),
            open_timeout: default_open_timeout(),
            test_requests: default_test_requests(),
            min_requests: default_min_requests(),
        }
    }
}

fn default_failure_threshold() -> u32 {
    5 // 5 failures trigger circuit open
}

fn default_failure_window() -> u64 {
    60 // 60 seconds
}

fn default_open_timeout() -> u64 {
    30 // 30 seconds
}

fn default_test_requests() -> u32 {
    3 // 3 test requests in half-open state
}

fn default_min_requests() -> u32 {
    10 // minimum 10 requests before circuit breaker activates
}

/// Load balancer that manages target selection
pub struct LoadBalancer {
    targets: Vec<Target>,
    strategy: LoadBalancingStrategy,
    /// Current position for round-robin strategies
    current_position: Arc<Mutex<usize>>,
    /// Weighted round-robin state
    weighted_state: Arc<Mutex<WeightedRoundRobinState>>,
    /// Connection tracking for least-connections
    connection_tracker: Arc<Mutex<ConnectionTracker>>,
    /// Sticky session mappings for client-to-backend affinity
    sticky_sessions: Arc<Mutex<HashMap<u64, String>>>,
    /// Dynamic health status tracking (overrides target.healthy)
    health_status: Arc<Mutex<HashMap<String, bool>>>,
    /// Circuit breakers for targets
    circuit_breakers: Arc<Mutex<HashMap<String, CircuitBreaker>>>,
}

// Implement Send and Sync for LoadBalancer since all its fields are thread-safe
unsafe impl Send for LoadBalancer {}
unsafe impl Sync for LoadBalancer {}

/// State for weighted round-robin algorithm
#[derive(Debug)]
struct WeightedRoundRobinState {
    current_weights: Vec<u32>,
    gcd_weight: u32,
    current_position: usize,
}

impl WeightedRoundRobinState {
    fn new(targets: &[Target]) -> Self {
        let current_weights: Vec<u32> = targets
            .iter()
            .map(|t| t.weight)
            .collect();
        let gcd_weight = Self::gcd_of_weights(&current_weights);

        Self {
            current_weights: current_weights.clone(),
            gcd_weight,
            current_position: 0,
        }
    }

    fn gcd_of_weights(weights: &[u32]) -> u32 {
        weights.iter().fold(0, |acc, &x| Self::gcd(acc, x))
    }

    fn gcd(a: u32, b: u32) -> u32 {
        if b == 0 { a } else { Self::gcd(b, a % b) }
    }
}

impl LoadBalancer {
    /// Create a new load balancer with the given targets and strategy
    pub fn new(targets: Vec<Target>, strategy: LoadBalancingStrategy) -> Self {
        let weighted_state = Arc::new(Mutex::new(WeightedRoundRobinState::new(&targets)));

        Self {
            targets,
            strategy,
            current_position: Arc::new(Mutex::new(0)),
            weighted_state,
            connection_tracker: Arc::new(Mutex::new(ConnectionTracker::new())),
            sticky_sessions: Arc::new(Mutex::new(HashMap::new())),
            health_status: Arc::new(Mutex::new(HashMap::new())),
            circuit_breakers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if a target is healthy (considers both static and dynamic health status)
    fn is_target_healthy(&self, target: &Target) -> bool {
        let health_status = self.health_status.lock().unwrap();

        // Check dynamic health status first, fall back to static if not set
        match health_status.get(&target.url) {
            Some(&dynamic_health) => dynamic_health,
            None => target.healthy, // Use static health status as default
        }
    }

    /// Select the next target based on the load balancing strategy
    pub fn select_target(&self) -> Option<&Target> {
        let healthy_targets: Vec<&Target> = self.targets
            .iter()
            .filter(|target| self.is_target_healthy(target))
            .collect();

        if healthy_targets.is_empty() {
            return None;
        }

        match self.strategy {
            LoadBalancingStrategy::RoundRobin => self.round_robin_select(&healthy_targets),
            LoadBalancingStrategy::WeightedRoundRobin => self.weighted_round_robin_select(),
            LoadBalancingStrategy::Random => self.random_select(&healthy_targets),
            LoadBalancingStrategy::LeastConnections =>
                self.least_connections_select(&healthy_targets),
        }
    }

    /// Round-robin target selection
    fn round_robin_select(&self, healthy_targets: &[&Target]) -> Option<&Target> {
        let mut position = self.current_position.lock().unwrap();
        let target = healthy_targets[*position % healthy_targets.len()];
        *position += 1;

        // Find the target in our original targets vec
        self.targets.iter().find(|t| t.url == target.url)
    }

    /// Weighted round-robin target selection
    fn weighted_round_robin_select(&self) -> Option<&Target> {
        let mut state = self.weighted_state.lock().unwrap();

        loop {
            state.current_position = (state.current_position + 1) % self.targets.len();

            if state.current_position == 0 {
                // Store gcd_weight to avoid borrowing issues
                let gcd_weight = state.gcd_weight;

                // Decrease all current weights by GCD
                for weight in &mut state.current_weights {
                    if *weight >= gcd_weight {
                        *weight -= gcd_weight;
                    }
                }

                // If all weights are 0, reset them
                if state.current_weights.iter().all(|&w| w == 0) {
                    state.current_weights = self.targets
                        .iter()
                        .map(|t| t.weight)
                        .collect();
                }
            }

            let target = &self.targets[state.current_position];
            if self.is_target_healthy(target) && state.current_weights[state.current_position] > 0 {
                return Some(target);
            }

            // Prevent infinite loop
            if state.current_weights.iter().all(|&w| w == 0) {
                break;
            }
        }

        // Fallback to round-robin if weighted selection fails
        drop(state);
        let healthy_targets: Vec<&Target> = self.targets
            .iter()
            .filter(|target| self.is_target_healthy(target))
            .collect();
        self.round_robin_select(&healthy_targets)
    }

    /// Random target selection
    fn random_select(&self, healthy_targets: &[&Target]) -> Option<&Target> {
        if healthy_targets.is_empty() {
            return None;
        }

        // Use a simple deterministic random selection based on current time
        use std::time::{ SystemTime, UNIX_EPOCH };
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
        let index = (seed as usize) % healthy_targets.len();
        let target = healthy_targets[index];

        // Find the target in our original targets vec
        self.targets.iter().find(|t| t.url == target.url)
    }

    /// Least connections target selection
    fn least_connections_select(&self, healthy_targets: &[&Target]) -> Option<&Target> {
        if healthy_targets.is_empty() {
            return None;
        }

        let tracker = self.connection_tracker.lock().unwrap();

        let min_connections_target = healthy_targets
            .iter()
            .min_by_key(|target| tracker.get_connections(&target.url))
            .copied()?;

        // Find the target in our original targets vec
        self.targets.iter().find(|t| t.url == min_connections_target.url)
    }

    /// Mark the start of a request to a target (for connection tracking)
    pub fn start_request(&self, target_url: &str) {
        let mut tracker = self.connection_tracker.lock().unwrap();
        tracker.increment(target_url);
    }

    /// Mark the end of a request to a target (for connection tracking)
    pub fn end_request(&self, target_url: &str) {
        let mut tracker = self.connection_tracker.lock().unwrap();
        tracker.decrement(target_url);
    }

    /// Mark a target as healthy or unhealthy (thread-safe)
    pub fn set_target_health(&self, target_url: &str, healthy: bool) {
        let mut health_status = self.health_status.lock().unwrap();
        health_status.insert(target_url.to_string(), healthy);

        tracing::info!(
            target_url = %target_url,
            healthy = healthy,
            "Health status updated for target"
        );
    }

    /// Get all targets
    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    /// Get the load balancing strategy
    pub fn strategy(&self) -> &LoadBalancingStrategy {
        &self.strategy
    }

    /// Get healthy targets count
    pub fn healthy_targets_count(&self) -> usize {
        self.targets
            .iter()
            .filter(|t| self.is_target_healthy(t))
            .count()
    }

    /// Get connection count for a target
    pub fn get_connection_count(&self, target_url: &str) -> u32 {
        let tracker = self.connection_tracker.lock().unwrap();
        tracker.get_connections(target_url)
    }

    /// Select target with sticky session support based on client identifier
    pub fn select_target_sticky(&self, client_id: &str) -> Option<&Target> {
        let healthy_targets: Vec<&Target> = self.targets
            .iter()
            .filter(|target| self.is_target_healthy(target))
            .collect();

        if healthy_targets.is_empty() {
            return None;
        }

        // Hash the client identifier to get consistent target selection
        let client_hash = self.hash_client_id(client_id);

        // Check if we have an existing sticky session for this client
        {
            let sticky_sessions = self.sticky_sessions.lock().unwrap();
            if let Some(target_url) = sticky_sessions.get(&client_hash) {
                // Return the sticky target if it's still healthy
                if
                    let Some(target) = self.targets
                        .iter()
                        .find(|t| &t.url == target_url && self.is_target_healthy(t))
                {
                    return Some(target);
                }
                // If the sticky target is unhealthy, we'll select a new one below
            }
        }

        // No existing sticky session or target is unhealthy - select new target
        let selected_target = match self.strategy {
            LoadBalancingStrategy::RoundRobin => self.round_robin_select(&healthy_targets),
            LoadBalancingStrategy::WeightedRoundRobin => self.weighted_round_robin_select(),
            LoadBalancingStrategy::Random => self.random_select(&healthy_targets),
            LoadBalancingStrategy::LeastConnections =>
                self.least_connections_select(&healthy_targets),
        };

        // Store the new sticky session mapping
        if let Some(target) = selected_target {
            let mut sticky_sessions = self.sticky_sessions.lock().unwrap();
            sticky_sessions.insert(client_hash, target.url.clone());
        }

        selected_target
    }

    /// Hash a client identifier for consistent target selection
    fn hash_client_id(&self, client_id: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        client_id.hash(&mut hasher);
        hasher.finish()
    }

    /// Clear sticky session for a client (useful when connection ends)
    pub fn clear_sticky_session(&self, client_id: &str) {
        let client_hash = self.hash_client_id(client_id);
        let mut sticky_sessions = self.sticky_sessions.lock().unwrap();
        sticky_sessions.remove(&client_hash);
    }

    /// Get the sticky session target for a client (if any)
    pub fn get_sticky_target(&self, client_id: &str) -> Option<String> {
        let client_hash = self.hash_client_id(client_id);
        let sticky_sessions = self.sticky_sessions.lock().unwrap();
        sticky_sessions.get(&client_hash).cloned()
    }

    /// Initialize circuit breaker for a target
    pub fn initialize_circuit_breaker(&self, target_url: &str, config: CircuitBreakerConfig) {
        let mut circuit_breakers = self.circuit_breakers.lock().unwrap();
        circuit_breakers.insert(target_url.to_string(), CircuitBreaker::new(config.clone()));

        tracing::info!(
            target_url = %target_url,
            enabled = config.enabled,
            failure_threshold = config.failure_threshold,
            "Circuit breaker initialized for target"
        );
    }

    /// Check if a request to a target should be allowed (circuit breaker check)
    pub fn allow_request(&self, target_url: &str) -> bool {
        let mut circuit_breakers = self.circuit_breakers.lock().unwrap();

        if let Some(circuit_breaker) = circuit_breakers.get_mut(target_url) {
            circuit_breaker.allow_request()
        } else {
            // No circuit breaker configured - allow request
            true
        }
    }

    /// Record a successful request for circuit breaker
    pub fn record_success(&self, target_url: &str) {
        let mut circuit_breakers = self.circuit_breakers.lock().unwrap();

        if let Some(circuit_breaker) = circuit_breakers.get_mut(target_url) {
            circuit_breaker.record_success();

            tracing::debug!(
                target_url = %target_url,
                state = ?circuit_breaker.state,
                success_count = circuit_breaker.success_count,
                "Circuit breaker recorded success"
            );
        }
    }

    /// Record a failed request for circuit breaker
    pub fn record_failure(&self, target_url: &str) {
        let mut circuit_breakers = self.circuit_breakers.lock().unwrap();

        if let Some(circuit_breaker) = circuit_breakers.get_mut(target_url) {
            circuit_breaker.record_failure();

            tracing::warn!(
                target_url = %target_url,
                state = ?circuit_breaker.state,
                failure_count = circuit_breaker.failure_count,
                "Circuit breaker recorded failure"
            );
        }
    }

    /// Get circuit breaker statistics for monitoring
    pub fn get_circuit_breaker_stats(&self, target_url: &str) -> Option<CircuitBreakerStats> {
        let circuit_breakers = self.circuit_breakers.lock().unwrap();
        circuit_breakers.get(target_url).map(|cb| cb.get_stats())
    }

    /// Get all circuit breaker statistics
    pub fn get_all_circuit_breaker_stats(&self) -> HashMap<String, CircuitBreakerStats> {
        let circuit_breakers = self.circuit_breakers.lock().unwrap();
        circuit_breakers
            .iter()
            .map(|(url, cb)| (url.clone(), cb.get_stats()))
            .collect()
    }

    /// Enhanced target selection that respects circuit breaker state
    pub fn select_target_with_circuit_breaker(&self) -> Option<&Target> {
        let available_targets: Vec<&Target> = self.targets
            .iter()
            .filter(|target| {
                // Target must be healthy AND circuit breaker must allow requests
                self.is_target_healthy(target) && self.allow_request(&target.url)
            })
            .collect();

        if available_targets.is_empty() {
            tracing::warn!("No available targets (all unhealthy or circuit breakers open)");
            return None;
        }

        // Use existing load balancing logic on filtered targets
        match self.strategy {
            LoadBalancingStrategy::RoundRobin => self.round_robin_select(&available_targets),
            LoadBalancingStrategy::WeightedRoundRobin => {
                // For weighted, we need to revert to simple round-robin if circuit breakers filter targets
                self.round_robin_select(&available_targets)
            }
            LoadBalancingStrategy::Random => self.random_select(&available_targets),
            LoadBalancingStrategy::LeastConnections =>
                self.least_connections_select(&available_targets),
        }
    }
}
