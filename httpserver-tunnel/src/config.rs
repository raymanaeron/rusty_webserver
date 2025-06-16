// Phase 7.1 Tunnel Client Configuration
// Phase 7.2 Tunnel Server Configuration
// Configuration structures for both tunnel client and server settings

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::HashMap;

/// Tunnel client configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TunnelConfig {
    /// Enable tunnel client
    #[serde(default)]
    pub enabled: bool,

    /// Local port to tunnel (defaults to server port)
    pub local_port: Option<u16>,

    /// Local host to tunnel (defaults to localhost)
    #[serde(default = "default_local_host")]
    pub local_host: String,

    /// Tunnel endpoints configuration
    #[serde(default)]
    pub endpoints: Vec<TunnelEndpoint>,

    /// Authentication configuration
    #[serde(default)]
    pub auth: TunnelAuthConfig,

    /// Auto-reconnection settings
    #[serde(default)]
    pub reconnection: ReconnectionConfig,

    /// Status monitoring settings
    #[serde(default)]
    pub monitoring: MonitoringConfig,

    /// SSL/TLS settings for tunnel connections
    #[serde(default)]
    pub ssl: TunnelSslConfig,

    /// Tunnel server configuration (Phase 7.2)
    #[serde(default)]
    pub server: TunnelServerConfig,
}

/// Tunnel server configuration (Phase 7.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelServerConfig {
    /// Enable tunnel server mode
    #[serde(default)]
    pub enabled: bool,    /// Port for tunnel WebSocket connections
    #[serde(default = "default_tunnel_port")]
    pub tunnel_port: u16,

    /// Public HTTP port (where subdomain traffic is served)
    #[serde(default = "default_public_port")]
    pub public_port: u16,

    /// Public HTTPS port for SSL tunnel traffic
    #[serde(default = "default_public_https_port")]
    pub public_https_port: u16,

    /// Base domain for subdomains (e.g., "httpserver.io")
    #[serde(default = "default_base_domain")]
    pub base_domain: String,

    /// Maximum number of concurrent tunnels
    #[serde(default = "default_max_tunnels")]
    pub max_tunnels: u32,

    /// Subdomain allocation strategy
    #[serde(default)]
    pub subdomain_strategy: SubdomainStrategy,

    /// Authentication settings for tunnel connections
    #[serde(default)]
    pub auth: TunnelServerAuthConfig,

    /// Rate limiting settings
    #[serde(default)]
    pub rate_limiting: TunnelRateLimitConfig,    /// SSL/TLS settings for public endpoints
    #[serde(default)]
    pub ssl: TunnelServerSslConfig,

    /// Network configuration settings
    #[serde(default)]
    pub network: TunnelServerNetworkConfig,
}

/// Subdomain allocation strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubdomainStrategy {
    /// Random alphanumeric subdomain
    Random,
    /// User-specified subdomain (if available)
    UserSpecified,
    /// UUID-based subdomain
    Uuid,
}

/// Tunnel server authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TunnelServerAuthConfig {
    /// Require authentication for tunnel connections
    #[serde(default = "default_true")]
    pub required: bool,

    /// Valid API keys for tunnel access
    #[serde(default)]
    pub api_keys: Vec<String>,

    /// JWT secret for token validation
    pub jwt_secret: Option<String>,

    /// Token expiration time in seconds
    #[serde(default = "default_token_expiry")]
    pub token_expiry: u64,
    
    /// Enable JWT token-based authentication
    #[serde(default = "default_false")]
    pub jwt_enabled: bool,
    
    /// Enable user registration
    #[serde(default = "default_false")]
    pub user_registration_enabled: bool,
    
    /// Enable API key rotation
    #[serde(default = "default_false")]
    pub api_key_rotation_enabled: bool,
    
    /// API key rotation interval in hours
    #[serde(default = "default_key_rotation_hours")]
    pub api_key_rotation_hours: u64,
    
    /// Admin API keys for user management
    #[serde(default)]
    pub admin_keys: Vec<String>,
}

/// Tunnel rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelRateLimitConfig {
    /// Enable rate limiting
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Maximum requests per minute per tunnel
    #[serde(default = "default_requests_per_minute")]
    pub requests_per_minute: u32,

    /// Maximum concurrent connections per tunnel
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_connections: u32,

    /// Maximum bandwidth per tunnel (bytes per second)
    #[serde(default = "default_max_bandwidth")]
    pub max_bandwidth_bps: u64,
}

/// Tunnel server SSL configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TunnelServerSslConfig {
    /// Enable SSL/TLS for public endpoints
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Wildcard certificate file path
    pub wildcard_cert_file: Option<PathBuf>,

    /// Wildcard private key file path
    pub wildcard_key_file: Option<PathBuf>,    /// Auto-redirect HTTP to HTTPS
    #[serde(default = "default_true")]
    pub redirect_http: bool,
}

/// Tunnel server network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelServerNetworkConfig {
    /// Bind address for tunnel server (e.g., "0.0.0.0" for all interfaces)
    #[serde(default = "default_bind_address")]
    pub bind_address: String,

    /// Bind address for public traffic (e.g., "0.0.0.0" for all interfaces)
    #[serde(default = "default_public_bind_address")]
    pub public_bind_address: String,

    /// Enable IPv6 support
    #[serde(default = "default_false")]
    pub ipv6_enabled: bool,

    /// TCP keepalive settings
    #[serde(default = "default_true")]
    pub tcp_keepalive: bool,

    /// TCP keepalive idle time in seconds
    #[serde(default = "default_tcp_keepalive_idle")]
    pub tcp_keepalive_idle: u64,

    /// TCP keepalive interval in seconds
    #[serde(default = "default_tcp_keepalive_interval")]
    pub tcp_keepalive_interval: u64,

    /// TCP keepalive probes
    #[serde(default = "default_tcp_keepalive_probes")]
    pub tcp_keepalive_probes: u32,

    /// Socket reuse address option
    #[serde(default = "default_true")]
    pub socket_reuse_address: bool,

    /// Socket reuse port option
    #[serde(default = "default_false")]
    pub socket_reuse_port: bool,
}

/// Tunnel endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelEndpoint {
    /// Tunnel server URL (e.g., wss://tunnel.httpserver.io)
    pub server_url: String,

    /// Requested subdomain (optional, server may assign)
    pub subdomain: Option<String>,

    /// Custom domain (for custom domain tunnels)
    pub custom_domain: Option<String>,

    /// Tunnel protocol version
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,

    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,

    /// Keep-alive interval in seconds
    #[serde(default = "default_keepalive_interval")]
    pub keepalive_interval: u64,

    /// Maximum concurrent connections through this tunnel
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

/// Tunnel authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TunnelAuthConfig {
    /// Authentication method (api_key, token, certificate)
    #[serde(default = "default_auth_method")]
    pub method: String,

    /// API key for authentication
    pub api_key: Option<String>,

    /// Authentication token
    pub token: Option<String>,

    /// Client certificate for mutual TLS
    pub cert_file: Option<PathBuf>,

    /// Client private key
    pub key_file: Option<PathBuf>,

    /// User account information
    pub user: Option<String>,

    /// Additional authentication headers
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Token refresh settings
    #[serde(default)]
    pub token_refresh: TokenRefreshConfig,
}

/// Token refresh configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenRefreshConfig {
    /// Enable automatic token refresh
    #[serde(default)]
    pub enabled: bool,

    /// Refresh endpoint URL
    pub refresh_url: Option<String>,

    /// Refresh interval in seconds
    #[serde(default = "default_refresh_interval")]
    pub interval: u64,

    /// Refresh token
    pub refresh_token: Option<String>,
}

/// Auto-reconnection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconnectionConfig {
    /// Enable auto-reconnection
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Initial retry delay in seconds
    #[serde(default = "default_initial_retry_delay")]
    pub initial_delay: u64,

    /// Maximum retry delay in seconds
    #[serde(default = "default_max_retry_delay")]
    pub max_delay: u64,

    /// Backoff multiplier
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,

    /// Maximum retry attempts (0 = unlimited)
    #[serde(default)]
    pub max_attempts: u32,

    /// Jitter factor for retry delays
    #[serde(default = "default_jitter_factor")]
    pub jitter_factor: f64,
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_delay: default_initial_retry_delay(),
            max_delay: default_max_retry_delay(),
            backoff_multiplier: default_backoff_multiplier(),
            max_attempts: 0,
            jitter_factor: default_jitter_factor(),
        }
    }
}

/// Status monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable status monitoring
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Health check interval in seconds
    #[serde(default = "default_health_interval")]
    pub health_interval: u64,

    /// Connection metrics collection
    #[serde(default = "default_true")]
    pub collect_metrics: bool,

    /// Status update callback URL
    pub status_webhook: Option<String>,

    /// Log tunnel events
    #[serde(default = "default_true")]
    pub log_events: bool,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            health_interval: default_health_interval(),
            collect_metrics: true,
            status_webhook: None,
            log_events: true,
        }
    }
}

/// SSL/TLS configuration for tunnel connections
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TunnelSslConfig {
    /// Verify server certificates
    #[serde(default = "default_true")]
    pub verify_server: bool,

    /// Custom CA certificate bundle
    pub ca_file: Option<PathBuf>,

    /// Client certificate for mutual TLS
    pub client_cert: Option<PathBuf>,

    /// Client private key
    pub client_key: Option<PathBuf>,

    /// SNI hostname override
    pub sni_hostname: Option<String>,

    /// ALPN protocols
    #[serde(default)]
    pub alpn_protocols: Vec<String>,
}

// Default value functions
fn default_local_host() -> String {
    "127.0.0.1".to_string()
}

fn default_protocol_version() -> String {
    "1.0".to_string()
}

fn default_connection_timeout() -> u64 {
    30 // 30 seconds
}

fn default_keepalive_interval() -> u64 {
    30 // 30 seconds
}

fn default_max_connections() -> u32 {
    100
}

fn default_auth_method() -> String {
    "api_key".to_string()
}

fn default_refresh_interval() -> u64 {
    3600 // 1 hour
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_initial_retry_delay() -> u64 {
    1 // 1 second
}

fn default_max_retry_delay() -> u64 {
    300 // 5 minutes
}

fn default_backoff_multiplier() -> f64 {
    2.0
}

fn default_jitter_factor() -> f64 {
    0.1
}

fn default_health_interval() -> u64 {
    30 // 30 seconds
}

fn default_tunnel_port() -> u16 { 8443 }
fn default_public_port() -> u16 { 80 }
fn default_public_https_port() -> u16 { 443 }
fn default_base_domain() -> String { "httpserver.io".to_string() }
fn default_max_tunnels() -> u32 { 1000 }
fn default_requests_per_minute() -> u32 { 1000 }
fn default_max_concurrent() -> u32 { 100 }
fn default_max_bandwidth() -> u64 { 10_485_760 } // 10 MB/s
fn default_token_expiry() -> u64 { 86400 } // 24 hours
fn default_key_rotation_hours() -> u64 { 168 } // 7 days

// Network configuration defaults
fn default_bind_address() -> String { "0.0.0.0".to_string() }
fn default_public_bind_address() -> String { "0.0.0.0".to_string() }
fn default_tcp_keepalive_idle() -> u64 { 600 }
fn default_tcp_keepalive_interval() -> u64 { 60 }
fn default_tcp_keepalive_probes() -> u32 { 9 }

impl Default for TunnelServerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tunnel_port: default_tunnel_port(),
            public_port: default_public_port(),
            public_https_port: default_public_https_port(),
            base_domain: default_base_domain(),
            max_tunnels: default_max_tunnels(),
            subdomain_strategy: SubdomainStrategy::Random,            auth: TunnelServerAuthConfig::default(),
            rate_limiting: TunnelRateLimitConfig::default(),
            ssl: TunnelServerSslConfig::default(),            network: TunnelServerNetworkConfig::default(),
        }
    }
}

impl Default for TunnelServerNetworkConfig {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
            public_bind_address: default_public_bind_address(),
            ipv6_enabled: default_false(),
            tcp_keepalive: default_true(),
            tcp_keepalive_idle: default_tcp_keepalive_idle(),
            tcp_keepalive_interval: default_tcp_keepalive_interval(),
            tcp_keepalive_probes: default_tcp_keepalive_probes(),
            socket_reuse_address: default_true(),
            socket_reuse_port: default_false(),
        }
    }
}

impl Default for SubdomainStrategy {
    fn default() -> Self {
        SubdomainStrategy::Random
    }
}

impl Default for TunnelRateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_minute: default_requests_per_minute(),
            max_concurrent_connections: default_max_concurrent(),
            max_bandwidth_bps: default_max_bandwidth(),
        }
    }
}
