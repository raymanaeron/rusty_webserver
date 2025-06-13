// Health check integration with load balancer (HTTP and WebSocket)
use std::sync::Arc;
use httpserver_config::{WebSocketHealthConfig, HttpHealthConfig};
use httpserver_balancer::LoadBalancer;
use crate::websocket_health::WebSocketHealthMonitor;
use crate::http_health::HttpHealthMonitor;

/// Health check integration manager
pub struct HealthCheckIntegration {
    load_balancer: Arc<LoadBalancer>,
    websocket_health_monitor: Option<WebSocketHealthMonitor>,
    http_health_monitor: Option<HttpHealthMonitor>,
}

impl HealthCheckIntegration {
    /// Create a new health check integration
    pub fn new(load_balancer: Arc<LoadBalancer>) -> Self {
        Self {
            load_balancer,
            websocket_health_monitor: None,
            http_health_monitor: None,
        }
    }

    /// Start HTTP health monitoring with load balancer integration
    pub async fn start_http_health_monitoring(&mut self, config: HttpHealthConfig) -> Result<(), String> {
        // Extract target URLs from the load balancer
        let targets: Vec<String> = self.load_balancer.targets()
            .iter()
            .map(|target| target.url.clone())
            .collect();

        if targets.is_empty() {
            return Err("No targets available for HTTP health monitoring".to_string());
        }

        // Create the health monitor
        let monitor = HttpHealthMonitor::new(config, targets);
        
        // Start monitoring with callback to update load balancer health
        let load_balancer_clone = self.load_balancer.clone();
        let _handle = monitor.start_monitoring_with_callback(move |target_url, is_healthy| {
            load_balancer_clone.set_target_health(target_url, is_healthy);
        }).await;

        self.http_health_monitor = Some(monitor);
        
        println!("HTTP health monitoring started for {} targets", 
            self.load_balancer.targets().len());
        
        Ok(())
    }

    /// Start WebSocket health monitoring with load balancer integration
    pub async fn start_websocket_health_monitoring(&mut self, config: WebSocketHealthConfig) -> Result<(), String> {
        // Extract target URLs from the load balancer
        let targets: Vec<String> = self.load_balancer.targets()
            .iter()
            .map(|target| target.url.clone())
            .collect();

        if targets.is_empty() {
            return Err("No targets available for health monitoring".to_string());
        }

        // Create the health monitor
        let monitor = WebSocketHealthMonitor::new(config, targets);
        
        // Start monitoring with callback to update load balancer health
        let load_balancer_clone = self.load_balancer.clone();
        let _handle = monitor.start_monitoring_with_callback(move |target_url, is_healthy| {
            load_balancer_clone.set_target_health(target_url, is_healthy);
        }).await;        self.websocket_health_monitor = Some(monitor);
        
        println!("WebSocket health monitoring started for {} targets", 
            self.load_balancer.targets().len());
        
        Ok(())
    }

    /// Get the current health status of all targets
    pub fn get_health_summary(&self) -> HealthSummary {
        let total_targets = self.load_balancer.targets().len();
        let healthy_targets = self.load_balancer.healthy_targets_count();
        
        HealthSummary {
            total_targets,
            healthy_targets,
            unhealthy_targets: total_targets - healthy_targets,
            monitoring_enabled: self.websocket_health_monitor.is_some() || self.http_health_monitor.is_some(),
        }
    }
}

/// Summary of health check status
#[derive(Debug, Clone)]
pub struct HealthSummary {
    pub total_targets: usize,
    pub healthy_targets: usize,
    pub unhealthy_targets: usize,
    pub monitoring_enabled: bool,
}

impl std::fmt::Display for HealthSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Health Summary: {}/{} healthy targets, monitoring: {}", 
            self.healthy_targets, 
            self.total_targets,
            if self.monitoring_enabled { "enabled" } else { "disabled" }
        )
    }
}
