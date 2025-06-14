// Phase 7.3 Tunnel Protocol Implementation
// Bidirectional communication protocol for HTTP request/response forwarding through WebSocket tunnels

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Tunnel protocol message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TunnelMessage {
    /// Client authentication request
    Auth {
        token: String,
        subdomain: Option<String>,
        protocol_version: String,
    },
    /// Server authentication response
    AuthResponse {
        success: bool,
        assigned_subdomain: Option<String>,
        error: Option<String>,
    },
    /// HTTP request forwarding from server to client
    HttpRequest {
        id: String,
        method: String,
        path: String,
        headers: HashMap<String, String>,
        body: Option<Vec<u8>>,
        client_ip: String,
    },
    /// HTTP response forwarding from client to server
    HttpResponse {
        id: String,
        status: u16,
        headers: HashMap<String, String>,
        body: Option<Vec<u8>>,
    },
    /// Tunnel heartbeat/keepalive
    Ping {
        timestamp: u64,
    },
    /// Heartbeat response
    Pong {
        timestamp: u64,
    },
    /// Error message
    Error {
        code: u16,
        message: String,
    },
    /// Tunnel metrics/status
    Status {
        connections: u32,
        bytes_sent: u64,
        bytes_received: u64,
    },
    /// SSL/TLS connection establishment for passthrough
    SslConnect {
        id: String,
        initial_data: Option<Vec<u8>>,
    },
    /// SSL/TLS data forwarding
    SslData {
        id: String,
        data: Vec<u8>,
    },
    /// SSL/TLS connection close
    SslClose {
        id: String,
    },
}

/// Tunnel protocol handler
#[derive(Debug)]
pub struct TunnelProtocol {
    protocol_version: String,
}

impl TunnelProtocol {
    /// Create new tunnel protocol instance
    pub fn new() -> Self {
        Self {
            protocol_version: "1.0".to_string(),
        }
    }

    /// Create authentication message
    pub fn create_auth_message(token: &str, subdomain: Option<&str>) -> TunnelMessage {
        TunnelMessage::Auth {
            token: token.to_string(),
            subdomain: subdomain.map(|s| s.to_string()),
            protocol_version: "1.0".to_string(),
        }
    }

    /// Create HTTP request message
    pub fn create_http_request_message(
        method: &str,
        path: &str,
        headers: HashMap<String, String>,
        body: Option<Vec<u8>>,
        client_ip: &str,
    ) -> TunnelMessage {
        TunnelMessage::HttpRequest {
            id: Uuid::new_v4().to_string(),
            method: method.to_string(),
            path: path.to_string(),
            headers,
            body,
            client_ip: client_ip.to_string(),
        }
    }

    /// Create HTTP response message
    pub fn create_http_response_message(
        request_id: &str,
        status: u16,
        headers: HashMap<String, String>,
        body: Option<Vec<u8>>,
    ) -> TunnelMessage {
        TunnelMessage::HttpResponse {
            id: request_id.to_string(),
            status,
            headers,
            body,
        }
    }

    /// Create ping message
    pub fn create_ping_message() -> TunnelMessage {
        TunnelMessage::Ping {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Create pong message
    pub fn create_pong_message(ping_timestamp: u64) -> TunnelMessage {
        TunnelMessage::Pong {
            timestamp: ping_timestamp,
        }
    }

    /// Create error message
    pub fn create_error_message(code: u16, message: &str) -> TunnelMessage {
        TunnelMessage::Error {
            code,
            message: message.to_string(),
        }
    }

    /// Create SSL connect message
    pub fn create_ssl_connect_message(connection_id: &str, initial_data: Option<Vec<u8>>) -> TunnelMessage {
        TunnelMessage::SslConnect {
            id: connection_id.to_string(),
            initial_data,
        }
    }

    /// Create SSL data message
    pub fn create_ssl_data_message(connection_id: &str, data: Vec<u8>) -> TunnelMessage {
        TunnelMessage::SslData {
            id: connection_id.to_string(),
            data,
        }
    }

    /// Create SSL close message
    pub fn create_ssl_close_message(connection_id: &str) -> TunnelMessage {
        TunnelMessage::SslClose {
            id: connection_id.to_string(),
        }
    }

    /// Serialize message to JSON bytes
    pub fn serialize_message(message: &TunnelMessage) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(message)
    }

    /// Deserialize message from JSON bytes
    pub fn deserialize_message(data: &[u8]) -> Result<TunnelMessage, serde_json::Error> {
        serde_json::from_slice(data)
    }

    /// Validate protocol version compatibility
    pub fn is_compatible_version(&self, client_version: &str) -> bool {
        // For now, only support exact version match
        // Future: implement semantic versioning compatibility
        client_version == self.protocol_version
    }
}

impl Default for TunnelProtocol {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_auth_message() {
        let msg = TunnelProtocol::create_auth_message("test-token", Some("myapp"));
        match msg {
            TunnelMessage::Auth { token, subdomain, protocol_version } => {
                assert_eq!(token, "test-token");
                assert_eq!(subdomain, Some("myapp".to_string()));
                assert_eq!(protocol_version, "1.0");
            }
            _ => panic!("Expected Auth message"),
        }
    }

    #[test]
    fn test_serialize_deserialize() {
        let original = TunnelProtocol::create_ping_message();
        let serialized = TunnelProtocol::serialize_message(&original).unwrap();
        let deserialized = TunnelProtocol::deserialize_message(&serialized).unwrap();
        
        match (original, deserialized) {
            (TunnelMessage::Ping { timestamp: ts1 }, TunnelMessage::Ping { timestamp: ts2 }) => {
                assert_eq!(ts1, ts2);
            }
            _ => panic!("Serialization/deserialization failed"),
        }
    }
}
