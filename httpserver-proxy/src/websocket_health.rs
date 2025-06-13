// WebSocket health check implementation
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message as TungsteniteMessage};
use futures_util::{SinkExt, StreamExt};
use httpserver_config::WebSocketHealthConfig;
use tracing;

/// WebSocket health checker
pub struct WebSocketHealthChecker {
    config: WebSocketHealthConfig,
}

impl WebSocketHealthChecker {
    pub fn new(config: WebSocketHealthConfig) -> Self {
        Self { config }
    }    /// Check if a WebSocket target is healthy
    #[tracing::instrument(skip(self), fields(timeout = self.config.timeout))]
    pub async fn check_health(&self, target_url: &str) -> bool {
        // Convert HTTP URL to WebSocket URL for health check
        let ws_url = self.convert_to_websocket_url(target_url);
        let health_url = format!("{}{}", ws_url, self.config.path);

        tracing::debug!(
            target_url = %target_url,
            health_url = %health_url,
            timeout = self.config.timeout,
            "Starting WebSocket health check"
        );

        match timeout(
            Duration::from_secs(self.config.timeout),
            self.perform_health_check(&health_url)
        ).await {
            Ok(result) => {
                tracing::debug!(
                    health_url = %health_url,
                    result = result,
                    "WebSocket health check completed"
                );
                result
            },
            Err(_) => {
                tracing::warn!(
                    health_url = %health_url,
                    timeout = self.config.timeout,
                    "WebSocket health check timeout"
                );
                false
            }
        }
    }    /// Perform the actual WebSocket health check
    #[tracing::instrument(skip(self))]
    async fn perform_health_check(&self, ws_url: &str) -> bool {
        match connect_async(ws_url).await {
            Ok((mut ws_stream, _)) => {
                tracing::debug!(
                    ws_url = %ws_url,
                    ping_message = %self.config.ping_message,
                    "WebSocket connection established, sending ping"
                );

                // Send ping message
                if let Err(e) = ws_stream.send(TungsteniteMessage::Text(self.config.ping_message.clone())).await {
                    tracing::error!(
                        ws_url = %ws_url,
                        error = %e,
                        "Failed to send WebSocket ping"
                    );
                    return false;
                }

                // Wait for pong or any response
                match ws_stream.next().await {
                    Some(Ok(TungsteniteMessage::Text(response))) => {
                        tracing::info!(
                            ws_url = %ws_url,
                            response = %response,
                            "WebSocket health check OK (text response)"
                        );
                        true
                    }
                    Some(Ok(TungsteniteMessage::Pong(_))) => {
                        tracing::info!(
                            ws_url = %ws_url,
                            "WebSocket health check OK (pong response)"
                        );
                        true
                    }
                    Some(Ok(_)) => {
                        tracing::info!(
                            ws_url = %ws_url,
                            "WebSocket health check OK (other message type)"
                        );
                        true
                    }
                    Some(Err(e)) => {
                        tracing::error!(
                            ws_url = %ws_url,
                            error = %e,
                            "WebSocket health check error"
                        );
                        false
                    }
                    None => {
                        tracing::warn!(
                            ws_url = %ws_url,
                            "WebSocket health check: no response received"
                        );
                        false
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    ws_url = %ws_url,
                    error = %e,
                    "Failed to connect to WebSocket for health check"
                );
                false
            }
        }
    }

    /// Convert HTTP/HTTPS URL to WebSocket URL
    fn convert_to_websocket_url(&self, http_url: &str) -> String {
        http_url.replace("http://", "ws://").replace("https://", "wss://")
    }
}

/// Background WebSocket health checker that runs periodic checks
pub struct WebSocketHealthMonitor {
    checker: WebSocketHealthChecker,
    targets: Vec<String>,
}

impl WebSocketHealthMonitor {
    pub fn new(config: WebSocketHealthConfig, targets: Vec<String>) -> Self {
        Self {
            checker: WebSocketHealthChecker::new(config),
            targets,
        }
    }

    /// Start background health monitoring with callback for health status updates
    pub async fn start_monitoring_with_callback<F>(&self, health_callback: F) -> tokio::task::JoinHandle<()>
    where
        F: Fn(&str, bool) + Send + Sync + 'static,
    {
        let checker = WebSocketHealthChecker::new(self.checker.config.clone());
        let targets = self.targets.clone();
        let interval = Duration::from_secs(self.checker.config.interval);

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            
            loop {
                interval_timer.tick().await;
                
                for target in &targets {
                    let is_healthy = checker.check_health(target).await;
                    println!("WebSocket health check for {}: {}", 
                        target, 
                        if is_healthy { "HEALTHY" } else { "UNHEALTHY" }
                    );
                    
                    // Update load balancer target health status via callback
                    health_callback(target, is_healthy);
                }
            }
        })
    }

    /// Start background health monitoring (legacy method for backward compatibility)
    pub async fn start_monitoring(&self) -> tokio::task::JoinHandle<()> {
        self.start_monitoring_with_callback(|target, is_healthy| {
            println!("WebSocket health update for {}: {}", 
                target, 
                if is_healthy { "HEALTHY" } else { "UNHEALTHY" }
            );
        }).await
    }
}
