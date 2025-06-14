// HTTP health check implementation
use std::time::Duration;
use tokio::time::timeout;
use reqwest;
use httpserver_config::HttpHealthConfig;
use tracing;

/// HTTP health checker using GET requests
pub struct HttpHealthChecker {
    config: HttpHealthConfig,
    client: reqwest::Client,
}

impl HttpHealthChecker {
    pub fn new(config: HttpHealthConfig) -> Self {
        let client = reqwest::Client
            ::builder()
            .timeout(Duration::from_secs(config.timeout))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self { config, client }
    }
    /// Check if an HTTP target is healthy via GET request
    #[tracing::instrument(skip(self), fields(timeout = self.config.timeout))]
    pub async fn check_health(&self, target_url: &str) -> bool {
        let health_url = format!("{}{}", target_url, self.config.path);

        tracing::debug!(
            target_url = %target_url,
            health_url = %health_url,
            timeout = self.config.timeout,
            "Starting HTTP health check"
        );

        match
            timeout(
                Duration::from_secs(self.config.timeout),
                self.perform_health_check(&health_url)
            ).await
        {
            Ok(result) => {
                tracing::debug!(
                    health_url = %health_url,
                    result = result,
                    "HTTP health check completed"
                );
                result
            }
            Err(_) => {
                tracing::warn!(
                    health_url = %health_url,
                    timeout = self.config.timeout,
                    "HTTP health check timeout"
                );
                false
            }
        }
    }
    /// Perform the actual HTTP health check
    #[tracing::instrument(skip(self))]
    async fn perform_health_check(&self, health_url: &str) -> bool {
        match self.client.get(health_url).send().await {
            Ok(response) => {
                let status = response.status();
                let is_healthy = status.is_success() || status == 200;

                if is_healthy {
                    tracing::info!(
                        health_url = %health_url,
                        status = %status,
                        "HTTP health check OK"
                    );
                } else {
                    tracing::warn!(
                        health_url = %health_url,
                        status = %status,
                        "HTTP health check failed"
                    );
                }

                is_healthy
            }
            Err(e) => {
                tracing::error!(
                    health_url = %health_url,
                    error = %e,
                    "Failed to perform HTTP health check"
                );
                false
            }
        }
    }
}

/// Background HTTP health checker that runs periodic checks
pub struct HttpHealthMonitor {
    checker: HttpHealthChecker,
    targets: Vec<String>,
}

impl HttpHealthMonitor {
    pub fn new(config: HttpHealthConfig, targets: Vec<String>) -> Self {
        Self {
            checker: HttpHealthChecker::new(config),
            targets,
        }
    }

    /// Start background health monitoring with callback for health status updates
    pub async fn start_monitoring_with_callback<F>(
        &self,
        health_callback: F
    ) -> tokio::task::JoinHandle<()>
        where F: Fn(&str, bool) + Send + Sync + 'static
    {
        let checker = HttpHealthChecker::new(self.checker.config.clone());
        let targets = self.targets.clone();
        let interval = Duration::from_secs(self.checker.config.interval);

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            loop {
                interval_timer.tick().await;

                for target in &targets {
                    let is_healthy = checker.check_health(target).await;
                    println!("HTTP health check for {}: {}", target, if is_healthy {
                        "HEALTHY"
                    } else {
                        "UNHEALTHY"
                    });

                    // Update load balancer target health status via callback
                    health_callback(target, is_healthy);
                }
            }
        })
    }

    /// Start background health monitoring (legacy method for backward compatibility)
    pub async fn start_monitoring(&self) -> tokio::task::JoinHandle<()> {
        self.start_monitoring_with_callback(|target, is_healthy| {
            println!("HTTP health update for {}: {}", target, if is_healthy {
                "HEALTHY"
            } else {
                "UNHEALTHY"
            });
        }).await
    }
}
