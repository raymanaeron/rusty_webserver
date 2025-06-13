use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use axum::{
    routing::get,
    Router,
    Json,
};
use serde_json::{json, Value};

// Re-export types from balancer crate
pub use httpserver_balancer::{LoadBalancingStrategy, Target};

/// Command line arguments
#[derive(Parser)]
#[command(name = "httpserver")]
#[command(about = "A simple cross-platform HTTP server and gateway")]
pub struct Args {
    /// Directory to serve files from
    #[arg(short, long, default_value = ".")]
    pub directory: PathBuf,

    /// Port to listen on
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    /// Configuration file for proxy routes (future feature)
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}

/// Server configuration (for future phases)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Static file serving configuration
    pub static_config: StaticConfig,
    
    /// Proxy routes (future feature)
    #[serde(default)]
    pub proxy: Vec<ProxyRoute>,
}

/// Static file serving configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticConfig {
    /// Directory to serve files from
    pub directory: PathBuf,
    
    /// Fallback file for SPA support
    #[serde(default = "default_fallback")]
    pub fallback: String,
}

/// Proxy route configuration (future feature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyRoute {
    /// Path pattern to match
    pub path: String,
    
    /// Single target URL (legacy - will be deprecated)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target: Option<String>,
    
    /// Multiple targets for load balancing
    #[serde(default)]
    pub targets: Vec<Target>,
    
    /// Load balancing strategy
    #[serde(default)]
    pub strategy: LoadBalancingStrategy,
    
    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    
    /// Enable sticky sessions for WebSocket connections
    #[serde(default)]
    pub sticky_sessions: bool,
    
    /// HTTP health check configuration
    #[serde(default)]
    pub http_health: Option<HttpHealthConfig>,

    /// WebSocket health check configuration
    #[serde(default)]
    pub websocket_health: Option<WebSocketHealthConfig>,
}

/// HTTP health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpHealthConfig {
    /// Health check interval in seconds
    #[serde(default = "default_health_interval")]
    pub interval: u64,
    
    /// Health check timeout in seconds
    #[serde(default = "default_health_timeout")]
    pub timeout: u64,
    
    /// HTTP health check path (relative to target URL)
    #[serde(default = "default_health_path")]
    pub path: String,
    
    /// Expected HTTP status codes (default: 200-299)
    #[serde(default = "default_expected_status_codes")]
    pub expected_status_codes: Vec<u16>,
}

fn default_expected_status_codes() -> Vec<u16> {
    vec![200] // Default to just 200 OK
}

fn default_health_interval() -> u64 {
    30 // 30 seconds
}

fn default_health_timeout() -> u64 {
    5 // 5 seconds
}

fn default_health_path() -> String {
    "/health".to_string()
}

fn default_ping_message() -> String {
    "ping".to_string()
}

/// WebSocket health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketHealthConfig {
    /// Health check interval in seconds
    #[serde(default = "default_health_interval")]
    pub interval: u64,
    
    /// Health check timeout in seconds
    #[serde(default = "default_health_timeout")]
    pub timeout: u64,
    
    /// WebSocket health check path (relative to target URL)
    #[serde(default = "default_health_path")]
    pub path: String,
    
    /// Ping message to send for WebSocket health checks
    #[serde(default = "default_ping_message")]
    pub ping_message: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            static_config: StaticConfig {
                directory: PathBuf::from("."),
                fallback: "index.html".to_string(),
            },
            proxy: Vec::new(),
        }
    }
}

fn default_fallback() -> String {
    "index.html".to_string()
}

fn default_timeout() -> u64 {
    30
}

impl Config {
    /// Load configuration from TOML file
    pub fn load_from_file(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        // Read the configuration file
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file '{}': {}", path.display(), e))?;
        
        // Parse TOML content
        let config: Config = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse TOML in '{}': {}", path.display(), e))?;
        
        // Validate configuration
        config.validate()?;
        
        println!("Loaded configuration from: {}", path.display());
        Ok(config)
    }
    
    /// Create config from command line arguments
    pub fn from_args(args: Args) -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = if let Some(config_path) = &args.config {
            Self::load_from_file(config_path)?
        } else {
            Self::default()
        };
        
        // Override with CLI arguments
        config.static_config.directory = args.directory;
        
        Ok(config)
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Validate static directory exists
        if !self.static_config.directory.exists() {
            return Err(format!(
                "Static directory does not exist: {}", 
                self.static_config.directory.display()
            ).into());
        }
        
        // Validate proxy routes
        for (index, route) in self.proxy.iter().enumerate() {
            route.validate(index)?;
        }
        
        println!("Configuration validation passed");
        Ok(())
    }
}

impl ProxyRoute {
    /// Get all targets for this route (handles both legacy single target and new multiple targets)
    pub fn get_targets(&self) -> Vec<Target> {
        if !self.targets.is_empty() {
            // New style: multiple targets
            self.targets.clone()
        } else if let Some(ref target_url) = self.target {
            // Legacy style: single target
            vec![Target::new(target_url.clone())]
        } else {
            // No targets configured
            vec![]
        }
    }
    
    /// Get the first target URL (for backward compatibility)
    pub fn get_primary_target(&self) -> Option<String> {
        if !self.targets.is_empty() {
            Some(self.targets[0].url.clone())
        } else {
            self.target.clone()
        }
    }
    
    /// Check if this route has multiple targets (load balancing enabled)
    pub fn has_multiple_targets(&self) -> bool {
        self.get_targets().len() > 1
    }
    
    /// Validate this proxy route
    fn validate(&self, index: usize) -> Result<(), Box<dyn std::error::Error>> {
        // Validate path pattern
        if self.path.is_empty() {
            return Err(format!("Proxy route {}: path cannot be empty", index).into());
        }
        
        let targets = self.get_targets();
        
        // Validate that at least one target is configured
        if targets.is_empty() {
            return Err(format!(
                "Proxy route {}: must have at least one target (use 'target' or 'targets')", 
                index
            ).into());
        }
        
        // Validate all target URLs
        for (target_index, target) in targets.iter().enumerate() {
            if target.url.is_empty() {
                return Err(format!(
                    "Proxy route {} target {}: URL cannot be empty", 
                    index, target_index
                ).into());
            }
            
            // Basic URL validation
            if !target.url.starts_with("http://") && !target.url.starts_with("https://") {
                return Err(format!(
                    "Proxy route {} target {}: must be a valid HTTP/HTTPS URL: {}", 
                    index, target_index, target.url
                ).into());
            }
            
            // Validate weight
            if target.weight == 0 {
                return Err(format!(
                    "Proxy route {} target {}: weight must be greater than 0", 
                    index, target_index
                ).into());
            }
        }
        
        // Validate timeout
        if self.timeout == 0 {
            return Err(format!("Proxy route {}: timeout must be greater than 0", index).into());
        }
        
        Ok(())
    }
}

/// Health status information for the config service
#[derive(Debug, Clone, Serialize)]
pub struct ConfigHealthStatus {
    pub status: String,
    pub service: String,
    pub config_loaded: bool,
    pub proxy_routes_count: usize,
    pub static_config_valid: bool,
}

/// Health endpoint handler for config service
pub async fn config_health() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "httpserver-config",
        "message": "Configuration parsing service operational"
    }))
}

/// Create config service health router
pub fn create_config_health_router() -> Router {
    Router::new()
        .route("/config/health", get(config_health))
        .route("/config/status", get(config_health))
}
