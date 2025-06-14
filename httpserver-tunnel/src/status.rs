// Phase 7.1 Tunnel Status Monitoring
// Tracks tunnel health, metrics, and provides status reporting

use crate::connection::ConnectionState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Duration;

/// Overall tunnel status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelStatus {
    /// Current connection state
    pub state: ConnectionState,
    
    /// Public URL if tunnel is active
    pub public_url: Option<String>,
    
    /// Tunnel ID if established
    pub tunnel_id: Option<String>,
    
    /// Connection health information
    pub health: ConnectionHealth,
    
    /// Connection metrics
    pub metrics: TunnelMetrics,
    
    /// Recent events
    pub recent_events: Vec<TunnelEvent>,
    
    /// Configuration summary
    pub config_summary: ConfigSummary,
}

/// Connection health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionHealth {
    /// Current connection state
    pub state: ConnectionState,
    
    /// Connection uptime
    pub uptime: Duration,
    
    /// Number of retry attempts
    pub retry_count: u32,
    
    /// Last connection error
    pub last_error: Option<String>,
    
    /// Health score (0-100)
    pub health_score: u8,
    
    /// Last successful ping time
    pub last_ping: Option<DateTime<Utc>>,
    
    /// Average ping latency
    pub avg_ping_latency: Option<Duration>,
}

impl Default for ConnectionHealth {
    fn default() -> Self {
        Self {
            state: ConnectionState::Disconnected,
            uptime: Duration::default(),
            retry_count: 0,
            last_error: None,
            health_score: 0,
            last_ping: None,
            avg_ping_latency: None,
        }
    }
}

/// Connection metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelMetrics {
    /// Total connections made
    pub total_connections: u64,
    
    /// Successful connections
    pub successful_connections: u64,
    
    /// Failed connections
    pub failed_connections: u64,
    
    /// Total bytes transferred
    pub bytes_transferred: u64,
    
    /// Total HTTP requests processed
    pub http_requests: u64,
    
    /// HTTP responses sent
    pub http_responses: u64,
    
    /// Average response time
    pub avg_response_time: Option<Duration>,
    
    /// Connected clients on server
    pub connected_clients: u32,
    
    /// Server uptime
    pub server_uptime: Duration,
    
    /// Total ping/pong exchanges
    pub total_pings: u64,
    
    /// Last ping time
    pub last_ping_time: Option<DateTime<Utc>>,
    
    /// Connection start time
    pub connection_start: Option<DateTime<Utc>>,
    
    /// Recent latency measurements
    pub latency_history: VecDeque<Duration>,
    
    /// Error counts by type
    pub error_counts: std::collections::HashMap<String, u32>,
}

impl TunnelMetrics {
    /// Create new metrics instance
    pub fn new() -> Self {
        Self {
            total_connections: 0,
            successful_connections: 0,
            failed_connections: 0,
            bytes_transferred: 0,
            http_requests: 0,
            http_responses: 0,
            avg_response_time: None,
            connected_clients: 0,
            server_uptime: Duration::default(),
            total_pings: 0,
            last_ping_time: None,
            connection_start: None,
            latency_history: VecDeque::with_capacity(100), // Keep last 100 measurements
            error_counts: std::collections::HashMap::new(),
        }
    }

    /// Record successful connection
    pub fn record_connection_success(&mut self) {
        self.total_connections += 1;
        self.successful_connections += 1;
        self.connection_start = Some(Utc::now());
    }

    /// Record failed connection
    pub fn record_connection_failure(&mut self, error_type: &str) {
        self.total_connections += 1;
        self.failed_connections += 1;
        
        // Increment error count
        let count = self.error_counts.entry(error_type.to_string()).or_insert(0);
        *count += 1;
    }

    /// Record HTTP request
    pub fn record_http_request(&mut self) {
        self.http_requests += 1;
    }

    /// Record HTTP response with timing
    pub fn record_http_response(&mut self, response_time: Duration) {
        self.http_responses += 1;
        
        // Update average response time
        if let Some(current_avg) = self.avg_response_time {
            // Simple moving average
            let total_time = current_avg.as_nanos() * (self.http_responses - 1) as u128 + response_time.as_nanos();
            self.avg_response_time = Some(Duration::from_nanos((total_time / self.http_responses as u128) as u64));
        } else {
            self.avg_response_time = Some(response_time);
        }
    }

    /// Record ping latency
    pub fn record_ping_latency(&mut self, latency: Duration) {
        self.latency_history.push_back(latency);
        
        // Keep only last 100 measurements
        if self.latency_history.len() > 100 {
            self.latency_history.pop_front();
        }
    }

    /// Get connection success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_connections == 0 {
            return 0.0;
        }
        self.successful_connections as f64 / self.total_connections as f64
    }

    /// Get average latency from recent measurements
    pub fn avg_latency(&self) -> Option<Duration> {
        if self.latency_history.is_empty() {
            return None;
        }
        
        let total_nanos: u128 = self.latency_history.iter().map(|d| d.as_nanos()).sum();
        let avg_nanos = total_nanos / self.latency_history.len() as u128;
        Some(Duration::from_nanos(avg_nanos as u64))
    }

    /// Get requests per minute
    pub fn requests_per_minute(&self) -> f64 {
        if let Some(start_time) = self.connection_start {
            let uptime = Utc::now().signed_duration_since(start_time);
            let minutes = uptime.num_minutes() as f64;
            if minutes > 0.0 {
                return self.http_requests as f64 / minutes;
            }
        }
        0.0
    }
}

impl Default for TunnelMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Tunnel event for logging and monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelEvent {
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Event type
    pub event_type: TunnelEventType,
    
    /// Event message
    pub message: String,
    
    /// Additional event data
    pub data: std::collections::HashMap<String, serde_json::Value>,
}

/// Types of tunnel events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelEventType {
    ConnectionAttempt,
    ConnectionSuccess,
    ConnectionFailure,
    AuthenticationSuccess,
    AuthenticationFailure,
    TunnelEstablished,
    TunnelClosed,
    HttpRequest,
    HttpResponse,
    Ping,
    Pong,
    Error,
    Reconnection,
    StatusUpdate,
}

impl TunnelEvent {
    /// Create new tunnel event
    pub fn new(event_type: TunnelEventType, message: String) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type,
            message,
            data: std::collections::HashMap::new(),
        }
    }

    /// Create event with additional data
    pub fn with_data(
        event_type: TunnelEventType, 
        message: String, 
        data: std::collections::HashMap<String, serde_json::Value>
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type,
            message,
            data,
        }
    }
}

/// Configuration summary for status reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSummary {
    /// Tunnel server URL
    pub server_url: String,
    
    /// Local port being tunneled
    pub local_port: u16,
    
    /// Requested subdomain
    pub subdomain: Option<String>,
    
    /// Authentication method
    pub auth_method: String,
    
    /// Auto-reconnection enabled
    pub auto_reconnect: bool,
    
    /// SSL verification enabled
    pub ssl_verify: bool,
    
    /// Protocol version
    pub protocol_version: String,
}

/// Status monitor for tracking tunnel health and metrics
pub struct TunnelStatusMonitor {
    events: VecDeque<TunnelEvent>,
    max_events: usize,
    metrics: TunnelMetrics,
    health: ConnectionHealth,
}

impl TunnelStatusMonitor {
    /// Create new status monitor
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
            max_events: 1000, // Keep last 1000 events
            metrics: TunnelMetrics::new(),
            health: ConnectionHealth::default(),
        }
    }

    /// Record a new event
    pub fn record_event(&mut self, event: TunnelEvent) {
        tracing::info!(
            event_type = ?event.event_type,
            message = %event.message,
            "Tunnel event recorded"
        );

        self.events.push_back(event);
        
        // Keep only the most recent events
        if self.events.len() > self.max_events {
            self.events.pop_front();
        }
    }

    /// Update connection health
    pub fn update_health(&mut self, health: ConnectionHealth) {
        self.health = health;
    }

    /// Update metrics
    pub fn update_metrics(&mut self, metrics: TunnelMetrics) {
        self.metrics = metrics;
    }

    /// Get current status
    pub fn get_status(&self, public_url: Option<String>, tunnel_id: Option<String>, config_summary: ConfigSummary) -> TunnelStatus {
        TunnelStatus {
            state: self.health.state.clone(),
            public_url,
            tunnel_id,
            health: self.health.clone(),
            metrics: self.metrics.clone(),
            recent_events: self.events.iter().rev().take(50).cloned().collect(), // Last 50 events
            config_summary,
        }
    }

    /// Get events by type
    pub fn get_events_by_type(&self, event_type: TunnelEventType) -> Vec<&TunnelEvent> {
        self.events.iter()
            .filter(|event| std::mem::discriminant(&event.event_type) == std::mem::discriminant(&event_type))
            .collect()
    }

    /// Get recent errors
    pub fn get_recent_errors(&self, limit: usize) -> Vec<&TunnelEvent> {
        self.events.iter()
            .rev()
            .filter(|event| matches!(event.event_type, TunnelEventType::Error | TunnelEventType::ConnectionFailure | TunnelEventType::AuthenticationFailure))
            .take(limit)
            .collect()
    }

    /// Calculate health score based on recent performance
    pub fn calculate_health_score(&self) -> u8 {
        if matches!(self.health.state, ConnectionState::Connected | ConnectionState::Authenticated) {
            let mut score = 100u8;
            
            // Reduce score for recent failures
            let recent_errors = self.get_recent_errors(10);
            score = score.saturating_sub((recent_errors.len() * 10) as u8);
            
            // Reduce score for high retry count
            if self.health.retry_count > 5 {
                score = score.saturating_sub(20);
            }
            
            // Reduce score if no recent ping
            if let Some(last_ping) = self.health.last_ping {
                let time_since_ping = Utc::now().signed_duration_since(last_ping);
                if time_since_ping.num_minutes() > 5 {
                    score = score.saturating_sub(30);
                }
            } else {
                score = score.saturating_sub(20);
            }
            
            score
        } else {
            0
        }
    }

    /// Export metrics for external monitoring
    pub fn export_metrics(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut metrics = std::collections::HashMap::new();
        
        metrics.insert("total_connections".to_string(), serde_json::Value::Number(self.metrics.total_connections.into()));
        metrics.insert("successful_connections".to_string(), serde_json::Value::Number(self.metrics.successful_connections.into()));
        metrics.insert("failed_connections".to_string(), serde_json::Value::Number(self.metrics.failed_connections.into()));        metrics.insert("success_rate".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(self.metrics.success_rate()).unwrap_or(serde_json::Number::from(0))));
        metrics.insert("http_requests".to_string(), serde_json::Value::Number(self.metrics.http_requests.into()));
        metrics.insert("http_responses".to_string(), serde_json::Value::Number(self.metrics.http_responses.into()));
        metrics.insert("bytes_transferred".to_string(), serde_json::Value::Number(self.metrics.bytes_transferred.into()));
        metrics.insert("requests_per_minute".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(self.metrics.requests_per_minute()).unwrap_or(serde_json::Number::from(0))));
        metrics.insert("health_score".to_string(), serde_json::Value::Number(self.calculate_health_score().into()));
        
        // Add connection state
        metrics.insert("connection_state".to_string(), serde_json::Value::String(format!("{:?}", self.health.state)));
        
        // Add uptime
        metrics.insert("uptime_seconds".to_string(), serde_json::Value::Number(self.health.uptime.as_secs().into()));
        
        // Add retry count
        metrics.insert("retry_count".to_string(), serde_json::Value::Number(self.health.retry_count.into()));
          // Add average latency if available
        if let Some(avg_latency) = self.metrics.avg_latency() {
            metrics.insert("avg_latency_ms".to_string(), serde_json::Value::Number((avg_latency.as_millis() as u64).into()));
        }
        
        metrics
    }
}

impl Default for TunnelStatusMonitor {
    fn default() -> Self {
        Self::new()
    }
}
