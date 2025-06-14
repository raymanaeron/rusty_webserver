use clap::Parser;
use serde::{ Deserialize, Serialize };
use std::path::PathBuf;
use axum::{ routing::get, Router, Json };
use serde_json::{ json, Value };
use tracing;

// Re-export types from balancer crate
pub use httpserver_balancer::{ LoadBalancingStrategy, Target, CircuitBreakerConfig };

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

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,

    /// Application configuration
    #[serde(default)]
    pub application: ApplicationConfig,

    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,

    /// Tunnel client configuration (Phase 7.1)
    #[serde(default)]
    pub tunnel: TunnelConfig,
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

    /// Circuit breaker configuration
    #[serde(default)]
    pub circuit_breaker: Option<CircuitBreakerConfig>,

    /// Middleware configuration for this route
    #[serde(default)]
    pub middleware: Option<MiddlewareConfig>,

    /// SSL/TLS configuration for this route
    #[serde(default)]
    pub ssl: Option<RouteSslConfig>,
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

/// Middleware configuration for request/response processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewareConfig {
    /// Header injection configuration
    #[serde(default)]
    pub headers: Option<HeaderMiddlewareConfig>,

    /// Rate limiting configuration
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,

    /// Request/response transformation configuration
    #[serde(default)]
    pub transform: Option<TransformConfig>,

    /// Authentication middleware configuration
    #[serde(default)]
    pub auth: Option<AuthMiddlewareConfig>,

    /// Compression middleware configuration
    #[serde(default)]
    pub compression: Option<CompressionConfig>,
}

/// Header injection and modification middleware
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderMiddlewareConfig {
    /// Headers to add to requests before forwarding to backend
    #[serde(default)]
    pub request_headers: std::collections::HashMap<String, String>,

    /// Headers to add to responses before returning to client
    #[serde(default)]
    pub response_headers: std::collections::HashMap<String, String>,

    /// Headers to remove from requests before forwarding
    #[serde(default)]
    pub remove_request_headers: Vec<String>,

    /// Headers to remove from responses before returning
    #[serde(default)]
    pub remove_response_headers: Vec<String>,

    /// Override the Host header for backend requests
    #[serde(default)]
    pub override_host: Option<String>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per minute per client IP
    #[serde(default = "default_requests_per_minute")]
    pub requests_per_minute: u32,

    /// Rate limit window in seconds
    #[serde(default = "default_rate_window")]
    pub window_seconds: u32,

    /// Maximum concurrent requests per client IP
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: u32,

    /// Rate limit by header value (e.g., API key, user ID)
    #[serde(default)]
    pub limit_by_header: Option<String>,

    /// Custom rate limit response message
    #[serde(default = "default_rate_limit_message")]
    pub rate_limit_message: String,
}

/// Request/response transformation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformConfig {
    /// Request body transformations
    #[serde(default)]
    pub request: Option<RequestTransformConfig>,

    /// Response body transformations
    #[serde(default)]
    pub response: Option<ResponseTransformConfig>,
}

/// Request transformation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestTransformConfig {
    /// Replace text in request body
    #[serde(default)]
    pub replace_text: Vec<TextReplacement>,

    /// Add fields to JSON request body
    #[serde(default)]
    pub add_json_fields: std::collections::HashMap<String, serde_json::Value>,

    /// Remove fields from JSON request body
    #[serde(default)]
    pub remove_json_fields: Vec<String>,
}

/// Response transformation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseTransformConfig {
    /// Replace text in response body
    #[serde(default)]
    pub replace_text: Vec<TextReplacement>,

    /// Add fields to JSON response body
    #[serde(default)]
    pub add_json_fields: std::collections::HashMap<String, serde_json::Value>,

    /// Remove fields from JSON response body
    #[serde(default)]
    pub remove_json_fields: Vec<String>,
}

/// Text replacement configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextReplacement {
    /// Pattern to find (supports regex if regex_enabled is true)
    pub find: String,

    /// Replacement text
    pub replace: String,

    /// Whether to use regex for pattern matching
    #[serde(default)]
    pub regex_enabled: bool,
}

/// Authentication middleware configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthMiddlewareConfig {
    /// Bearer token to add to requests
    #[serde(default)]
    pub bearer_token: Option<String>,

    /// Basic auth credentials (username:password)
    #[serde(default)]
    pub basic_auth: Option<String>,

    /// Custom authentication header name and value
    #[serde(default)]
    pub custom_auth_header: Option<(String, String)>,

    /// API key header configuration
    #[serde(default)]
    pub api_key: Option<ApiKeyConfig>,
}

/// API key configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyConfig {
    /// Header name for API key
    pub header_name: String,

    /// API key value
    pub key_value: String,
}

/// Compression middleware configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Enable gzip compression
    #[serde(default = "default_gzip_enabled")]
    pub gzip: bool,

    /// Enable brotli compression
    #[serde(default = "default_brotli_enabled")]
    pub brotli: bool,

    /// Minimum response size to compress (in bytes)
    #[serde(default = "default_compression_threshold")]
    pub threshold_bytes: usize,

    /// Compression level (1-9 for gzip, 1-11 for brotli)
    #[serde(default = "default_compression_level")]
    pub level: u32,
}

/// SSL/TLS configuration for the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslConfig {
    /// Enable SSL/TLS termination
    #[serde(default)]
    pub enabled: bool,

    /// HTTPS port (default: 443)
    #[serde(default = "default_https_port")]
    pub https_port: u16,

    /// Certificate file path (PEM format)
    pub cert_file: Option<PathBuf>,

    /// Private key file path (PEM format)
    pub key_file: Option<PathBuf>,

    /// Certificate chain file path (optional, for intermediate certificates)
    pub cert_chain_file: Option<PathBuf>,

    /// Wildcard certificate configuration
    #[serde(default)]
    pub wildcard: Option<WildcardCertConfig>,

    /// Let's Encrypt configuration
    #[serde(default)]
    pub lets_encrypt: Option<LetsEncryptConfig>,

    /// Force HTTPS redirect (redirect HTTP to HTTPS) - legacy field
    #[serde(default = "default_force_https")]
    pub force_https: bool,

    /// HTTPS redirect configuration
    #[serde(default)]
    pub redirect: Option<SslRedirectConfig>,

    /// SSL protocol versions to support
    #[serde(default = "default_ssl_protocols")]
    pub protocols: Vec<String>,

    /// Cipher suites to allow (empty = use defaults)
    #[serde(default)]
    pub cipher_suites: Vec<String>,
}

/// SSL/TLS redirect configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslRedirectConfig {
    /// Enable HTTP to HTTPS redirection
    #[serde(default)]
    pub enabled: bool,

    /// Paths that don't require HTTPS redirect (e.g., health checks)
    #[serde(default)]
    pub exempt_paths: Vec<String>,
}

/// Wildcard certificate configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WildcardCertConfig {
    /// Domain for wildcard certificate (e.g., "*.httpserver.io")
    pub domain: String,

    /// Certificate file path for wildcard cert
    pub cert_file: PathBuf,

    /// Private key file path for wildcard cert
    pub key_file: PathBuf,

    /// Whether this wildcard cert covers tunnel subdomains
    #[serde(default)]
    pub covers_tunnels: bool,
}

/// Let's Encrypt configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LetsEncryptConfig {
    /// Enable Let's Encrypt automatic certificate generation
    #[serde(default)]
    pub enabled: bool,

    /// Email for Let's Encrypt registration
    pub email: String,

    /// Domain for which to obtain certificates
    pub domain: Option<String>,

    /// Use staging environment (for testing)
    #[serde(default)]
    pub staging: bool,

    /// DNS-01 challenge configuration for wildcard certificates
    #[serde(default)]
    pub dns_challenge: Option<DnsChallengeConfig>,
}

/// DNS-01 challenge configuration for wildcard certificates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsChallengeConfig {
    /// DNS provider (e.g., "cloudflare", "route53")
    pub provider: String,

    /// API credentials (provider-specific)
    pub credentials: std::collections::HashMap<String, String>,

    /// Challenge timeout in seconds
    #[serde(default = "default_dns_timeout")]
    pub timeout_seconds: u64,
}

/// SSL configuration for individual proxy routes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteSslConfig {
    /// Enable SSL termination for this route
    #[serde(default)]
    pub enabled: bool,

    /// Custom certificate for this route (overrides server default)
    pub cert_file: Option<PathBuf>,

    /// Custom private key for this route
    pub key_file: Option<PathBuf>,

    /// Forward to backend over HTTPS instead of HTTP
    #[serde(default)]
    pub backend_ssl: bool,

    /// Verify backend SSL certificates
    #[serde(default = "default_verify_backend")]
    pub verify_backend_ssl: bool,
}

// Default value functions for middleware configuration
fn default_requests_per_minute() -> u32 {
    100
}

fn default_rate_window() -> u32 {
    60
}

fn default_max_concurrent() -> u32 {
    10
}

fn default_rate_limit_message() -> String {
    "Rate limit exceeded. Please try again later.".to_string()
}

fn default_gzip_enabled() -> bool {
    true
}

fn default_brotli_enabled() -> bool {
    false
}

fn default_compression_threshold() -> usize {
    1024 // 1KB
}

fn default_compression_level() -> u32 {
    6 // Balanced compression level
}

/// Default value functions for SSL configuration
fn default_https_port() -> u16 {
    443
}

fn default_force_https() -> bool {
    false
}

fn default_ssl_protocols() -> Vec<String> {
    vec!["TLSv1.2".to_string(), "TLSv1.3".to_string()]
}

fn default_dns_timeout() -> u64 {
    300 // 5 minutes
}

fn default_verify_backend() -> bool {
    true
}

// Phase 7.1 - Tunnel Client Configuration
// Import tunnel configuration types
pub use httpserver_tunnel::{TunnelConfig, TunnelEndpoint, TunnelAuthConfig};

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (e.g., "debug", "info", "warn", "error")
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Enable file logging
    #[serde(default = "default_file_logging")]
    pub file_logging: bool,

    /// Log directory path (default: "./logs")
    #[serde(default = "default_logs_directory")]
    pub logs_directory: PathBuf,

    /// Log file size limit in MB (default: 10)
    #[serde(default = "default_file_size_mb")]
    pub file_size_mb: u64,

    /// Log retention in days (default: 30)
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,

    /// Log format ("json" or "text", default: "text")
    #[serde(default = "default_log_format")]
    pub format: String,

    /// Output mode ("both", "file", "console", default: "both")
    #[serde(default = "default_output_mode")]
    pub output_mode: String,

    /// Enable structured logging with additional fields
    #[serde(default = "default_structured_logging")]
    pub structured_logging: bool,

    /// Enable request ID generation for traceability
    #[serde(default = "default_enable_request_ids")]
    pub enable_request_ids: bool,

    /// Enable performance metrics logging
    #[serde(default = "default_enable_performance_metrics")]
    pub enable_performance_metrics: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file_logging: default_file_logging(),
            logs_directory: default_logs_directory(),
            file_size_mb: default_file_size_mb(),
            retention_days: default_retention_days(),
            format: default_log_format(),
            output_mode: default_output_mode(),
            structured_logging: default_structured_logging(),
            enable_request_ids: default_enable_request_ids(),
            enable_performance_metrics: default_enable_performance_metrics(),
        }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_file_logging() -> bool {
    true
}

fn default_logs_directory() -> PathBuf {
    PathBuf::from("./logs")
}

fn default_file_size_mb() -> u64 {
    10 // 10 MB
}

fn default_retention_days() -> u32 {
    30 // 30 days
}

fn default_log_format() -> String {
    "text".to_string()
}

fn default_output_mode() -> String {
    "both".to_string()
}

fn default_structured_logging() -> bool {
    true
}

fn default_enable_request_ids() -> bool {
    true
}

fn default_enable_performance_metrics() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            static_config: StaticConfig {
                directory: PathBuf::from("."),
                fallback: "index.html".to_string(),
            },
            proxy: Vec::new(),
            logging: LoggingConfig::default(),
            application: ApplicationConfig::default(),
            server: ServerConfig::default(),
            tunnel: TunnelConfig::default(),
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
        let content = std::fs
            ::read_to_string(path)
            .map_err(|e| format!("Failed to read config file '{}': {}", path.display(), e))?;

        // Parse TOML content
        let config: Config = toml
            ::from_str(&content)
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

    /// Load application configuration from app_config.toml file
    pub fn load_app_config() -> Result<Self, Box<dyn std::error::Error>> {
        let app_config_path = PathBuf::from("app_config.toml");

        if app_config_path.exists() {
            tracing::info!(
                config_file = %app_config_path.display(),
                "Loading application configuration from app_config.toml"
            );
            Self::load_from_file(&app_config_path)
        } else {
            tracing::warn!(
                config_file = %app_config_path.display(),
                "app_config.toml not found, using default configuration"
            );
            Ok(Self::default())
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Validate static directory exists
        if !self.static_config.directory.exists() {
            return Err(
                format!(
                    "Static directory does not exist: {}",
                    self.static_config.directory.display()
                ).into()
            );
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
            return Err(
                format!("Proxy route {}: must have at least one target (use 'target' or 'targets')", index).into()
            );
        }

        // Validate all target URLs
        for (target_index, target) in targets.iter().enumerate() {
            if target.url.is_empty() {
                return Err(
                    format!(
                        "Proxy route {} target {}: URL cannot be empty",
                        index,
                        target_index
                    ).into()
                );
            }

            // Basic URL validation
            if !target.url.starts_with("http://") && !target.url.starts_with("https://") {
                return Err(
                    format!(
                        "Proxy route {} target {}: must be a valid HTTP/HTTPS URL: {}",
                        index,
                        target_index,
                        target.url
                    ).into()
                );
            }

            // Validate weight
            if target.weight == 0 {
                return Err(
                    format!(
                        "Proxy route {} target {}: weight must be greater than 0",
                        index,
                        target_index
                    ).into()
                );
            }
        }

        // Validate timeout
        if self.timeout == 0 {
            return Err(format!("Proxy route {}: timeout must be greater than 0", index).into());
        }

        Ok(())
    }
}

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationConfig {
    /// Application name for logging context
    #[serde(default = "default_app_name")]
    pub name: String,

    /// Environment: "development", "staging", "production"
    #[serde(default = "default_environment")]
    pub environment: String,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Default port if not specified via command line
    #[serde(default = "default_server_port")]
    pub default_port: u16,

    /// Request timeout in seconds
    #[serde(default = "default_request_timeout")]
    pub request_timeout: u64,

    /// Maximum request body size in MB
    #[serde(default = "default_max_request_size_mb")]
    pub max_request_size_mb: u64,

    /// Enable health endpoints
    #[serde(default = "default_enable_health_endpoints")]
    pub enable_health_endpoints: bool,

    /// SSL/TLS configuration
    #[serde(default)]
    pub ssl: Option<SslConfig>,
}

fn default_app_name() -> String {
    "httpserver".to_string()
}

fn default_environment() -> String {
    "development".to_string()
}

fn default_server_port() -> u16 {
    8080
}

fn default_request_timeout() -> u64 {
    30
}

fn default_max_request_size_mb() -> u64 {
    10
}

fn default_enable_health_endpoints() -> bool {
    true
}

impl Default for ApplicationConfig {
    fn default() -> Self {
        Self {
            name: default_app_name(),
            environment: default_environment(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            default_port: default_server_port(),
            request_timeout: default_request_timeout(),
            max_request_size_mb: default_max_request_size_mb(),
            enable_health_endpoints: default_enable_health_endpoints(),
            ssl: None,
        }
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
    Json(
        json!({
        "status": "healthy",
        "service": "httpserver-config",
        "message": "Configuration parsing service operational"
    })
    )
}

/// Create config service health router
pub fn create_config_health_router() -> Router {
    Router::new()
        .route("/config/health", get(config_health))
        .route("/config/status", get(config_health))
}
