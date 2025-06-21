// Phase 7.1 Main Tunnel Client
// Orchestrates tunnel connections, manages multiple tunnels, and provides the main API

use crate::{TunnelError, TunnelResult};
use crate::auth::TunnelAuthenticator;
use crate::config::TunnelConfig;
use crate::connection::{TunnelConnection, ConnectionState};
use crate::status::{TunnelStatus, TunnelStatusMonitor, TunnelEvent, TunnelEventType, ConfigSummary};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, watch};
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration};
use tracing;

/// Main tunnel client that manages multiple tunnel connections
pub struct TunnelClient {
    config: TunnelConfig,
    authenticator: Arc<TunnelAuthenticator>,
    connections: Arc<RwLock<HashMap<String, Arc<TunnelConnection>>>>,
    status_monitor: Arc<RwLock<TunnelStatusMonitor>>,
      // Control channels
    shutdown_sender: Option<tokio::sync::broadcast::Sender<()>>,
    status_sender: watch::Sender<Vec<TunnelStatus>>,
    status_receiver: watch::Receiver<Vec<TunnelStatus>>,
    
    // Background tasks
    monitoring_task: Option<JoinHandle<()>>,
    health_check_task: Option<JoinHandle<()>>,
    
    // Client state
    is_running: Arc<RwLock<bool>>,
    local_port: u16,
    local_host: String,
}

impl TunnelClient {
    /// Create new tunnel client
    pub fn new(config: TunnelConfig, local_port: u16) -> TunnelResult<Self> {
        if !config.enabled {
            return Err(TunnelError::InvalidConfig("Tunnel client is disabled".to_string()));
        }

        if config.endpoints.is_empty() {
            return Err(TunnelError::InvalidConfig("No tunnel endpoints configured".to_string()));
        }

        let authenticator = Arc::new(TunnelAuthenticator::new(config.auth.clone())?);        let (status_sender, status_receiver) = watch::channel(Vec::new());
        let local_host = config.local_host.clone();

        Ok(Self {
            config,
            authenticator,
            connections: Arc::new(RwLock::new(HashMap::new())),
            status_monitor: Arc::new(RwLock::new(TunnelStatusMonitor::new())),
            shutdown_sender: None,
            status_sender,
            status_receiver,
            monitoring_task: None,
            health_check_task: None,
            is_running: Arc::new(RwLock::new(false)),
            local_port,
            local_host,
        })
    }

    /// Start the tunnel client and establish connections
    pub async fn start(&mut self) -> TunnelResult<()> {
        if *self.is_running.read().await {
            return Err(TunnelError::InvalidConfig("Tunnel client is already running".to_string()));
        }

        *self.is_running.write().await = true;

        tracing::info!(
            endpoint_count = self.config.endpoints.len(),
            local_port = self.local_port,
            "Starting tunnel client"
        );

        // Record startup event
        let event = TunnelEvent::new(
            TunnelEventType::ConnectionAttempt,
            "Tunnel client starting".to_string(),
        );
        self.status_monitor.write().await.record_event(event);

        // Validate authentication before starting connections
        for endpoint in &self.config.endpoints {
            match self.authenticator.validate_credentials(&endpoint.server_url).await {
                Ok(true) => {
                    tracing::info!(url = %endpoint.server_url, "Authentication validated");
                }
                Ok(false) => {
                    tracing::warn!(url = %endpoint.server_url, "Authentication validation failed");
                    return Err(TunnelError::AuthenticationFailed(
                        format!("Credentials validation failed for {}", endpoint.server_url)
                    ));
                }
                Err(e) => {
                    tracing::warn!(url = %endpoint.server_url, error = %e, "Could not validate credentials");
                    // Continue anyway - might be a temporary network issue
                }
            }
        }        // Set up shutdown channel
        let (shutdown_tx, _shutdown_rx) = tokio::sync::broadcast::channel(1);
        self.shutdown_sender = Some(shutdown_tx.clone());

        // Start connections for each endpoint
        let mut connection_tasks = Vec::new();
        
        for (index, endpoint) in self.config.endpoints.iter().enumerate() {
            let connection_id = format!("tunnel-{}", index);
              let connection = TunnelConnection::new(
                endpoint.clone(),
                self.authenticator.clone(),
                self.config.reconnection.clone(),
                self.local_port,
                &self.local_host,
            );

            let connection_arc = Arc::new(connection);
            self.connections.write().await.insert(connection_id.clone(), connection_arc.clone());            // Start connection in background
            let local_port = self.local_port;
            let local_host = self.local_host.clone();
            let status_monitor = self.status_monitor.clone();
            let mut shutdown_rx_clone = shutdown_tx.subscribe();
            let endpoint_clone = endpoint.clone();
            let authenticator_clone = self.authenticator.clone();
            let reconnection_clone = self.config.reconnection.clone();
            let connection_id_for_task = connection_id.clone();
              let task = tokio::spawn(async move {
                // Create a new connection instance for this task
                let mut connection = TunnelConnection::new(
                    endpoint_clone,
                    authenticator_clone,
                    reconnection_clone,
                    local_port,
                    &local_host,
                );
                
                tokio::select! {
                    result = connection.start() => {
                        match result {
                            Ok(_) => {
                                tracing::info!("Connection {} established successfully", connection_id_for_task);
                            }
                            Err(e) => {
                                let error_msg = e.to_string();
                                tracing::error!("Connection {} failed: {}", connection_id_for_task, error_msg);
                                
                                let event = TunnelEvent::new(
                                    TunnelEventType::ConnectionFailure,
                                    format!("Connection {} failed: {}", connection_id_for_task, error_msg),
                                );
                                status_monitor.write().await.record_event(event);
                            }
                        }
                    }
                    _ = shutdown_rx_clone.recv() => {
                        tracing::info!("Connection {} shutdown requested", connection_id_for_task);
                    }
                }
            });
            
            connection_tasks.push(task);
        }

        // Start monitoring tasks
        self.start_monitoring_tasks().await?;

        tracing::info!("Tunnel client started successfully");

        Ok(())
    }

    /// Stop the tunnel client and close all connections
    pub async fn stop(&mut self) -> TunnelResult<()> {
        if !*self.is_running.read().await {
            return Ok(());
        }

        tracing::info!("Stopping tunnel client");

        *self.is_running.write().await = false;        // Send shutdown signal
        if let Some(shutdown_tx) = &self.shutdown_sender {
            let _ = shutdown_tx.send(());
        }

        // Stop monitoring tasks
        if let Some(task) = self.monitoring_task.take() {
            task.abort();
        }

        if let Some(task) = self.health_check_task.take() {
            task.abort();
        }

        // Clear connections
        self.connections.write().await.clear();

        // Record shutdown event
        let event = TunnelEvent::new(
            TunnelEventType::StatusUpdate,
            "Tunnel client stopped".to_string(),
        );
        self.status_monitor.write().await.record_event(event);

        tracing::info!("Tunnel client stopped successfully");

        Ok(())
    }

    /// Start background monitoring tasks
    async fn start_monitoring_tasks(&mut self) -> TunnelResult<()> {
        let monitoring_config = &self.config.monitoring;

        if monitoring_config.enabled {
            // Start status monitoring task
            let connections = self.connections.clone();
            let status_monitor = self.status_monitor.clone();
            let status_sender = self.status_sender.clone();
            let config = self.config.clone();

            self.monitoring_task = Some(tokio::spawn(async move {
                let mut interval = interval(Duration::from_secs(10)); // Update every 10 seconds
                
                loop {
                    interval.tick().await;
                    
                    let mut statuses = Vec::new();
                    let connections_guard = connections.read().await;
                      for (_connection_id, connection) in connections_guard.iter() {
                        let _health = connection.get_health().await;
                        let _metrics = connection.get_metrics().await;
                        let public_url = connection.get_public_url().await;
                        let tunnel_id = connection.get_tunnel_id().await;
                        
                        // Create config summary
                        let endpoint = &config.endpoints[0]; // TODO: Handle multiple endpoints properly
                        let config_summary = ConfigSummary {
                            server_url: endpoint.server_url.clone(),
                            local_port: config.local_port.unwrap_or(8080),
                            subdomain: endpoint.subdomain.clone(),
                            auth_method: config.auth.method.clone(),
                            auto_reconnect: config.reconnection.enabled,
                            ssl_verify: config.ssl.verify_server,
                            protocol_version: endpoint.protocol_version.clone(),
                        };
                        
                        let status = status_monitor.read().await.get_status(
                            public_url,
                            tunnel_id,
                            config_summary,
                        );
                        
                        statuses.push(status);
                    }
                    
                    // Send status update
                    let _ = status_sender.send(statuses);
                }
            }));

            // Start health check task if configured
            if monitoring_config.health_interval > 0 {
                let connections = self.connections.clone();
                let status_monitor = self.status_monitor.clone();
                let health_interval = monitoring_config.health_interval;

                self.health_check_task = Some(tokio::spawn(async move {
                    let mut interval = interval(Duration::from_secs(health_interval));
                    
                    loop {
                        interval.tick().await;
                        
                        let connections_guard = connections.read().await;
                        
                        for (connection_id, connection) in connections_guard.iter() {
                            let health = connection.get_health().await;
                            
                            // Update health in status monitor
                            status_monitor.write().await.update_health(health.clone());
                            
                            // Log health status
                            tracing::debug!(
                                connection_id = %connection_id,
                                state = ?health.state,
                                uptime = ?health.uptime,
                                retry_count = health.retry_count,
                                "Health check update"
                            );
                        }
                    }
                }));
            }
        }

        Ok(())
    }

    /// Get current status of all tunnels
    pub async fn get_status(&self) -> Vec<TunnelStatus> {
        self.status_receiver.borrow().clone()
    }

    /// Get status of specific tunnel
    pub async fn get_tunnel_status(&self, connection_id: &str) -> Option<TunnelStatus> {
        let connections = self.connections.read().await;        if let Some(connection) = connections.get(connection_id) {
            let _health = connection.get_health().await;
            let _metrics = connection.get_metrics().await;
            let public_url = connection.get_public_url().await;
            let tunnel_id = connection.get_tunnel_id().await;
            
            // Create config summary (simplified)
            let config_summary = ConfigSummary {
                server_url: self.config.endpoints[0].server_url.clone(),
                local_port: self.local_port,
                subdomain: self.config.endpoints[0].subdomain.clone(),
                auth_method: self.config.auth.method.clone(),
                auto_reconnect: self.config.reconnection.enabled,
                ssl_verify: self.config.ssl.verify_server,
                protocol_version: self.config.endpoints[0].protocol_version.clone(),
            };
            
            let status = self.status_monitor.read().await.get_status(
                public_url,
                tunnel_id,
                config_summary,
            );
            
            Some(status)
        } else {
            None
        }
    }

    /// Get public URLs for all active tunnels
    pub async fn get_public_urls(&self) -> HashMap<String, String> {
        let mut urls = HashMap::new();
        let connections = self.connections.read().await;
        
        for (connection_id, connection) in connections.iter() {
            if let Some(url) = connection.get_public_url().await {
                urls.insert(connection_id.clone(), url);
            }
        }
        
        urls
    }

    /// Check if any tunnels are connected
    pub async fn is_connected(&self) -> bool {
        let connections = self.connections.read().await;
        
        for connection in connections.values() {
            let state = connection.get_state().await;
            if matches!(state, ConnectionState::Connected | ConnectionState::Authenticated) {
                return true;
            }
        }
        
        false
    }

    /// Get connection count
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Get active connection count
    pub async fn active_connection_count(&self) -> usize {
        let connections = self.connections.read().await;
        let mut count = 0;
        
        for connection in connections.values() {
            let state = connection.get_state().await;
            if matches!(state, ConnectionState::Connected | ConnectionState::Authenticated) {
                count += 1;
            }
        }
        
        count
    }

    /// Subscribe to status updates
    pub fn subscribe_status(&self) -> watch::Receiver<Vec<TunnelStatus>> {
        self.status_receiver.clone()
    }

    /// Get tunnel configuration
    pub fn get_config(&self) -> &TunnelConfig {
        &self.config
    }

    /// Check if tunnel client is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// Export metrics for external monitoring systems
    pub async fn export_metrics(&self) -> HashMap<String, serde_json::Value> {
        let mut all_metrics = HashMap::new();
        
        // Add client-level metrics
        all_metrics.insert("connection_count".to_string(), serde_json::Value::Number(self.connection_count().await.into()));
        all_metrics.insert("active_connections".to_string(), serde_json::Value::Number(self.active_connection_count().await.into()));
        all_metrics.insert("is_running".to_string(), serde_json::Value::Bool(self.is_running().await));
        all_metrics.insert("is_connected".to_string(), serde_json::Value::Bool(self.is_connected().await));
          // Add per-connection metrics
        let connections = self.connections.read().await;
        for (connection_id, connection) in connections.iter() {
            let _metrics = connection.get_metrics().await;
            let connection_metrics = self.status_monitor.read().await.export_metrics();
            
            for (key, value) in connection_metrics {
                all_metrics.insert(format!("{}_{}", connection_id, key), value);
            }
        }
        
        all_metrics
    }

    /// Force reconnection of all tunnels
    pub async fn reconnect_all(&self) -> TunnelResult<()> {
        tracing::info!("Forcing reconnection of all tunnels");
        
        // This would require implementing reconnection logic in TunnelConnection
        // For now, just log the request
        let event = TunnelEvent::new(
            TunnelEventType::Reconnection,
            "Manual reconnection requested".to_string(),
        );
        self.status_monitor.write().await.record_event(event);
        
        Ok(())
    }
}
