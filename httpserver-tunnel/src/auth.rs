// Phase 7.1 Tunnel Authentication
// Handles API key authentication, token management, and certificate-based auth

use crate::{TunnelError, TunnelResult};
use crate::config::TunnelAuthConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing;

/// Tunnel authentication manager
pub struct TunnelAuthenticator {
    config: TunnelAuthConfig,
    token_cache: RwLock<Option<AuthToken>>,
    last_refresh: RwLock<Option<Instant>>,
    http_client: reqwest::Client,
}

/// Authentication token with expiry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub token: String,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub token_type: String,
    pub scope: Option<String>,
}

/// Authentication credentials for tunnel connection
#[derive(Debug, Clone)]
pub struct TunnelCredentials {
    pub headers: HashMap<String, String>,
    pub query_params: HashMap<String, String>,
    pub auth_method: String,
}

impl TunnelAuthenticator {
    /// Create new tunnel authenticator
    pub fn new(config: TunnelAuthConfig) -> TunnelResult<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| TunnelError::InvalidConfig(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            token_cache: RwLock::new(None),
            last_refresh: RwLock::new(None),
            http_client,
        })
    }

    /// Get authentication credentials for tunnel connection
    pub async fn get_credentials(&self) -> TunnelResult<TunnelCredentials> {
        match self.config.method.as_str() {
            "api_key" => self.get_api_key_credentials().await,
            "token" => self.get_token_credentials().await,
            "certificate" => self.get_certificate_credentials().await,
            method => Err(TunnelError::InvalidConfig(format!("Unsupported auth method: {}", method))),
        }
    }

    /// Get API key based credentials
    async fn get_api_key_credentials(&self) -> TunnelResult<TunnelCredentials> {
        let api_key = self.config.api_key
            .as_ref()
            .ok_or_else(|| TunnelError::AuthenticationFailed("API key not configured".to_string()))?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {}", api_key));
        headers.insert("User-Agent".to_string(), "httpserver-tunnel/1.0".to_string());

        // Add any custom authentication headers
        for (key, value) in &self.config.headers {
            headers.insert(key.clone(), value.clone());
        }

        // Add user info if configured
        if let Some(user) = &self.config.user {
            headers.insert("X-Tunnel-User".to_string(), user.clone());
        }

        Ok(TunnelCredentials {
            headers,
            query_params: HashMap::new(),
            auth_method: "api_key".to_string(),
        })
    }

    /// Get token based credentials with auto-refresh
    async fn get_token_credentials(&self) -> TunnelResult<TunnelCredentials> {
        // Check if we need to refresh the token
        if self.should_refresh_token().await {
            self.refresh_token().await?;
        }

        let token = if let Some(cached_token) = self.get_cached_token().await {
            cached_token.token
        } else if let Some(static_token) = &self.config.token {
            static_token.clone()
        } else {
            return Err(TunnelError::AuthenticationFailed("No token available".to_string()));
        };

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {}", token));
        headers.insert("User-Agent".to_string(), "httpserver-tunnel/1.0".to_string());

        // Add custom headers
        for (key, value) in &self.config.headers {
            headers.insert(key.clone(), value.clone());
        }

        Ok(TunnelCredentials {
            headers,
            query_params: HashMap::new(),
            auth_method: "token".to_string(),
        })
    }

    /// Get certificate based credentials
    async fn get_certificate_credentials(&self) -> TunnelResult<TunnelCredentials> {
        // Certificate authentication is handled at the TLS level
        // Just provide basic headers here
        let mut headers = HashMap::new();
        headers.insert("User-Agent".to_string(), "httpserver-tunnel/1.0".to_string());
        headers.insert("X-Auth-Method".to_string(), "certificate".to_string());

        if let Some(user) = &self.config.user {
            headers.insert("X-Tunnel-User".to_string(), user.clone());
        }

        Ok(TunnelCredentials {
            headers,
            query_params: HashMap::new(),
            auth_method: "certificate".to_string(),
        })
    }

    /// Check if token needs refreshing
    async fn should_refresh_token(&self) -> bool {
        if !self.config.token_refresh.enabled {
            return false;
        }

        // Check if we have a cached token that's about to expire
        if let Some(token) = self.get_cached_token().await {
            if let Some(expires_at) = token.expires_at {
                let now = chrono::Utc::now();
                let expires_soon = expires_at - chrono::Duration::minutes(5); // Refresh 5 minutes before expiry
                if now >= expires_soon {
                    return true;
                }
            }
        }

        // Check if it's time for periodic refresh
        if let Some(last_refresh) = *self.last_refresh.read().await {
            let refresh_interval = Duration::from_secs(self.config.token_refresh.interval);
            if last_refresh.elapsed() >= refresh_interval {
                return true;
            }
        } else {
            // Never refreshed before
            return true;
        }

        false
    }

    /// Refresh authentication token
    async fn refresh_token(&self) -> TunnelResult<()> {
        let refresh_config = &self.config.token_refresh;
        
        if !refresh_config.enabled {
            return Err(TunnelError::AuthenticationFailed("Token refresh not enabled".to_string()));
        }

        let refresh_url = refresh_config.refresh_url
            .as_ref()
            .ok_or_else(|| TunnelError::InvalidConfig("Refresh URL not configured".to_string()))?;

        let refresh_token = refresh_config.refresh_token
            .as_ref()
            .ok_or_else(|| TunnelError::AuthenticationFailed("Refresh token not available".to_string()))?;

        tracing::info!("Refreshing authentication token");

        let mut request_body = HashMap::new();
        request_body.insert("grant_type", "refresh_token");
        request_body.insert("refresh_token", refresh_token);

        let response = self.http_client
            .post(refresh_url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| TunnelError::NetworkError(format!("Token refresh request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(TunnelError::AuthenticationFailed(
                format!("Token refresh failed: {} - {}", status, error_text)
            ));
        }

        let token_response: AuthToken = response
            .json()
            .await
            .map_err(|e| TunnelError::ProtocolError(format!("Invalid token response: {}", e)))?;

        // Cache the new token
        *self.token_cache.write().await = Some(token_response);
        *self.last_refresh.write().await = Some(Instant::now());

        tracing::info!("Authentication token refreshed successfully");
        Ok(())
    }

    /// Get cached token if available and valid
    async fn get_cached_token(&self) -> Option<AuthToken> {
        let token_guard = self.token_cache.read().await;
        if let Some(token) = token_guard.as_ref() {
            // Check if token is still valid
            if let Some(expires_at) = token.expires_at {
                let now = chrono::Utc::now();
                if now < expires_at {
                    return Some(token.clone());
                }
            } else {
                // No expiry, assume valid
                return Some(token.clone());
            }
        }
        None
    }

    /// Validate credentials by making a test request
    pub async fn validate_credentials(&self, tunnel_server_url: &str) -> TunnelResult<bool> {
        let credentials = self.get_credentials().await?;
        
        // Construct validation endpoint URL
        let validation_url = format!("{}/api/v1/auth/validate", 
            tunnel_server_url.replace("wss://", "https://").replace("ws://", "http://"));

        let mut request = self.http_client.get(&validation_url);

        // Add authentication headers
        for (key, value) in credentials.headers {
            request = request.header(&key, &value);
        }

        let response = request
            .send()
            .await
            .map_err(|e| TunnelError::NetworkError(format!("Validation request failed: {}", e)))?;

        let is_valid = response.status().is_success();
        
        if is_valid {
            tracing::info!("Tunnel authentication validated successfully");
        } else {
            tracing::warn!(status = %response.status(), "Tunnel authentication validation failed");
        }

        Ok(is_valid)
    }

    /// Get authentication method
    pub fn get_auth_method(&self) -> &str {
        &self.config.method
    }

    /// Check if certificate files are configured and exist
    pub fn has_client_certificate(&self) -> bool {
        self.config.cert_file.is_some() && self.config.key_file.is_some()
    }

    /// Get client certificate file path
    pub fn get_cert_file(&self) -> Option<&std::path::Path> {
        self.config.cert_file.as_deref()
    }

    /// Get client key file path
    pub fn get_key_file(&self) -> Option<&std::path::Path> {
        self.config.key_file.as_deref()
    }
}
