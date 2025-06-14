// Phase 7.1 Tunnel Client - Local HTTP Server Integration
// Phase 7.2 Tunnel Server - Public HTTP Server Integration
// Built-in tunnel client/server that handles secure WebSocket connections for public URL exposure

pub mod client;
pub mod config;
pub mod auth;
pub mod connection;
pub mod status;
pub mod server;  // Phase 7.2: Tunnel Server
pub mod protocol;  // Phase 7.3: Tunnel Protocol
pub mod subdomain;  // Phase 7.2: Subdomain Management

// Re-export main types for easy usage
pub use client::TunnelClient;
pub use server::TunnelServer;  // Phase 7.2
pub use config::{TunnelConfig, TunnelEndpoint, TunnelAuthConfig, TunnelServerConfig};
pub use auth::TunnelAuthenticator;
pub use connection::{TunnelConnection, ConnectionState, ReconnectionStrategy};
pub use status::{TunnelStatus, ConnectionHealth, TunnelMetrics};
pub use protocol::{TunnelMessage, TunnelProtocol};  // Phase 7.3

use std::error::Error;
use std::fmt;

/// Tunnel client errors
#[derive(Debug)]
pub enum TunnelError {
    /// Connection failed
    ConnectionFailed(String),    /// Authentication failed
    AuthenticationFailed(String),
    /// Invalid configuration
    InvalidConfig(String),
    /// Configuration error
    ConfigError(String),
    /// Network error
    NetworkError(String),
    /// Protocol error
    ProtocolError(String),
    /// Tunnel server unavailable
    ServerUnavailable(String),
    /// Internal error
    InternalError(String),
    /// Validation error
    ValidationError(String),
    /// Conflict error
    ConflictError(String),
    /// Serialization error
    SerializationError(String),
    /// IO error
    IoError(String),
}

impl fmt::Display for TunnelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TunnelError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),            TunnelError::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            TunnelError::InvalidConfig(msg) => write!(f, "Invalid configuration: {}", msg),
            TunnelError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            TunnelError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            TunnelError::ProtocolError(msg) => write!(f, "Protocol error: {}", msg),
            TunnelError::ServerUnavailable(msg) => write!(f, "Server unavailable: {}", msg),
            TunnelError::InternalError(msg) => write!(f, "Internal error: {}", msg),
            TunnelError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            TunnelError::ConflictError(msg) => write!(f, "Conflict error: {}", msg),
            TunnelError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            TunnelError::IoError(msg) => write!(f, "IO error: {}", msg),
        }
    }
}

impl Error for TunnelError {}

/// Tunnel operation result type
pub type TunnelResult<T> = Result<T, TunnelError>;
