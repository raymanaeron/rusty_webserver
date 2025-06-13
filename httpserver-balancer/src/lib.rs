use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use axum::{
    routing::get,
    Router,
    Json,
};
use serde_json::{json, Value};

/// Health endpoint handler for load balancer service
pub async fn balancer_health() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "httpserver-balancer",
        "message": "Load balancing service operational"
    }))
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
        let current_weights: Vec<u32> = targets.iter().map(|t| t.weight).collect();
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
        let healthy_targets: Vec<&Target> = self.targets.iter()
            .filter(|target| self.is_target_healthy(target))
            .collect();
            
        if healthy_targets.is_empty() {
            return None;
        }
        
        match self.strategy {
            LoadBalancingStrategy::RoundRobin => self.round_robin_select(&healthy_targets),
            LoadBalancingStrategy::WeightedRoundRobin => self.weighted_round_robin_select(),
            LoadBalancingStrategy::Random => self.random_select(&healthy_targets),
            LoadBalancingStrategy::LeastConnections => self.least_connections_select(&healthy_targets),
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
                    state.current_weights = self.targets.iter().map(|t| t.weight).collect();
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
        let healthy_targets: Vec<&Target> = self.targets.iter()
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
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
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
        
        println!("Health status updated for {}: {}", target_url, 
            if healthy { "HEALTHY" } else { "UNHEALTHY" });
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
        self.targets.iter().filter(|t| self.is_target_healthy(t)).count()
    }
    
    /// Get connection count for a target
    pub fn get_connection_count(&self, target_url: &str) -> u32 {
        let tracker = self.connection_tracker.lock().unwrap();
        tracker.get_connections(target_url)
    }
    
    /// Select target with sticky session support based on client identifier
    pub fn select_target_sticky(&self, client_id: &str) -> Option<&Target> {
        let healthy_targets: Vec<&Target> = self.targets.iter()
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
                if let Some(target) = self.targets.iter().find(|t| &t.url == target_url && self.is_target_healthy(t)) {
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
            LoadBalancingStrategy::LeastConnections => self.least_connections_select(&healthy_targets),
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
}
