use axum::{
    extract::Request,
    response::Response,
    http::{HeaderName, HeaderValue},
    body::Body,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
    net::SocketAddr,
};
use tracing;
use serde_json::Value;

// Re-export middleware configuration types
pub use httpserver_config::{
    MiddlewareConfig, HeaderMiddlewareConfig, RateLimitConfig, TransformConfig,
    RequestTransformConfig, ResponseTransformConfig, TextReplacement,
    AuthMiddlewareConfig, ApiKeyConfig, CompressionConfig
};

/// Middleware processor that applies various transformations to requests and responses
pub struct MiddlewareProcessor {
    /// Rate limiting state
    rate_limiter: Arc<Mutex<RateLimiter>>,
}

/// Rate limiting implementation
#[derive(Debug)]
struct RateLimiter {
    /// Client request counters (IP -> (count, window_start))
    request_counts: HashMap<String, (u32, Instant)>,
    
    /// Active connection counts per client
    active_connections: HashMap<String, u32>,
}

/// Middleware processing error
#[derive(Debug)]
pub enum MiddlewareError {
    /// Rate limit exceeded
    RateLimitExceeded(String),
    /// Header processing error
    HeaderError(String),
    /// Body transformation error
    TransformError(String),
    /// Authentication error
    AuthError(String),
    /// Compression error
    CompressionError(String),
}

impl std::fmt::Display for MiddlewareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MiddlewareError::RateLimitExceeded(msg) => write!(f, "Rate limit exceeded: {}", msg),
            MiddlewareError::HeaderError(msg) => write!(f, "Header error: {}", msg),
            MiddlewareError::TransformError(msg) => write!(f, "Transform error: {}", msg),
            MiddlewareError::AuthError(msg) => write!(f, "Authentication error: {}", msg),
            MiddlewareError::CompressionError(msg) => write!(f, "Compression error: {}", msg),
        }
    }
}

impl std::error::Error for MiddlewareError {}

impl MiddlewareProcessor {
    /// Create a new middleware processor
    pub fn new() -> Self {
        Self {
            rate_limiter: Arc::new(Mutex::new(RateLimiter {
                request_counts: HashMap::new(),
                active_connections: HashMap::new(),
            })),
        }
    }
    
    /// Process request middleware (headers, auth, rate limiting, transformation)
    pub async fn process_request(
        &self,
        mut req: Request<Body>,
        client_ip: &SocketAddr,
        middleware_config: &MiddlewareConfig,
    ) -> Result<Request<Body>, MiddlewareError> {
        let client_id = client_ip.ip().to_string();
        
        // Apply rate limiting first
        if let Some(rate_config) = &middleware_config.rate_limit {
            self.check_rate_limit(&client_id, rate_config)?;
        }
        
        // Apply authentication middleware
        if let Some(auth_config) = &middleware_config.auth {
            req = self.apply_auth_headers(req, auth_config)?;
        }
        
        // Apply header modifications
        if let Some(header_config) = &middleware_config.headers {
            req = self.apply_request_headers(req, header_config)?;
        }
        
        // Apply request transformations
        if let Some(transform_config) = &middleware_config.transform {
            if let Some(request_config) = &transform_config.request {
                req = self.transform_request_body(req, request_config).await?;
            }
        }
        
        tracing::debug!(
            client_ip = %client_ip,
            "Request middleware processing completed"
        );
        
        Ok(req)
    }
    
    /// Process response middleware (headers, transformation, compression)
    pub async fn process_response(
        &self,
        mut response: Response<Body>,
        middleware_config: &MiddlewareConfig,
    ) -> Result<Response<Body>, MiddlewareError> {
        // Apply response header modifications
        if let Some(header_config) = &middleware_config.headers {
            response = self.apply_response_headers(response, header_config)?;
        }
        
        // Apply response transformations
        if let Some(transform_config) = &middleware_config.transform {
            if let Some(response_config) = &transform_config.response {
                response = self.transform_response_body(response, response_config).await?;
            }
        }
        
        // Apply compression
        if let Some(compression_config) = &middleware_config.compression {
            response = self.apply_compression(response, compression_config).await?;
        }
        
        tracing::debug!("Response middleware processing completed");
        
        Ok(response)
    }
    
    /// Check rate limiting for a client
    fn check_rate_limit(
        &self,
        client_id: &str,
        rate_config: &RateLimitConfig,
    ) -> Result<(), MiddlewareError> {
        let mut limiter = self.rate_limiter.lock().unwrap();
        
        let now = Instant::now();
        let window_duration = Duration::from_secs(rate_config.window_seconds as u64);
        
        // Check and update request count
        let (count, window_start) = limiter.request_counts
            .get(client_id)
            .copied()
            .unwrap_or((0, now));
        
        // Reset window if expired
        let (new_count, new_window_start) = if now.duration_since(window_start) > window_duration {
            (1, now)
        } else {
            (count + 1, window_start)
        };
        
        // Check rate limit
        if new_count > rate_config.requests_per_minute {
            tracing::warn!(
                client_id = %client_id,
                count = new_count,
                limit = rate_config.requests_per_minute,
                "Rate limit exceeded"
            );
            return Err(MiddlewareError::RateLimitExceeded(rate_config.rate_limit_message.clone()));
        }
        
        // Check concurrent connections
        let active_count = limiter.active_connections.get(client_id).copied().unwrap_or(0);
        if active_count >= rate_config.max_concurrent {
            tracing::warn!(
                client_id = %client_id,
                active_connections = active_count,
                max_concurrent = rate_config.max_concurrent,
                "Concurrent connection limit exceeded"
            );
            return Err(MiddlewareError::RateLimitExceeded(
                "Too many concurrent connections".to_string()
            ));
        }
        
        // Update counters
        limiter.request_counts.insert(client_id.to_string(), (new_count, new_window_start));
        limiter.active_connections.insert(client_id.to_string(), active_count + 1);
        
        Ok(())
    }
    
    /// Apply authentication headers to request
    fn apply_auth_headers(
        &self,
        mut req: Request<Body>,
        auth_config: &AuthMiddlewareConfig,
    ) -> Result<Request<Body>, MiddlewareError> {
        let headers = req.headers_mut();
        
        // Bearer token
        if let Some(token) = &auth_config.bearer_token {
            let auth_value = format!("Bearer {}", token);
            headers.insert(
                "authorization",
                HeaderValue::from_str(&auth_value)
                    .map_err(|e| MiddlewareError::AuthError(format!("Invalid bearer token: {}", e)))?
            );
            tracing::debug!("Added Bearer token to request");
        }
          // Basic auth
        if let Some(basic_auth) = &auth_config.basic_auth {
            use base64::{Engine as _, engine::general_purpose};
            let encoded = general_purpose::STANDARD.encode(basic_auth);
            let auth_value = format!("Basic {}", encoded);
            headers.insert(
                "authorization",
                HeaderValue::from_str(&auth_value)
                    .map_err(|e| MiddlewareError::AuthError(format!("Invalid basic auth: {}", e)))?
            );
            tracing::debug!("Added Basic auth to request");
        }
        
        // Custom auth header
        if let Some((header_name, header_value)) = &auth_config.custom_auth_header {
            headers.insert(
                HeaderName::from_bytes(header_name.as_bytes())
                    .map_err(|e| MiddlewareError::AuthError(format!("Invalid header name: {}", e)))?,
                HeaderValue::from_str(header_value)
                    .map_err(|e| MiddlewareError::AuthError(format!("Invalid header value: {}", e)))?
            );
            tracing::debug!(header_name = %header_name, "Added custom auth header to request");
        }
        
        // API key
        if let Some(api_key_config) = &auth_config.api_key {
            headers.insert(
                HeaderName::from_bytes(api_key_config.header_name.as_bytes())
                    .map_err(|e| MiddlewareError::AuthError(format!("Invalid API key header name: {}", e)))?,
                HeaderValue::from_str(&api_key_config.key_value)
                    .map_err(|e| MiddlewareError::AuthError(format!("Invalid API key value: {}", e)))?
            );
            tracing::debug!(
                header_name = %api_key_config.header_name,
                "Added API key to request"
            );
        }
        
        Ok(req)
    }
    
    /// Apply header modifications to request
    fn apply_request_headers(
        &self,
        mut req: Request<Body>,
        header_config: &HeaderMiddlewareConfig,
    ) -> Result<Request<Body>, MiddlewareError> {
        let headers = req.headers_mut();
        
        // Remove headers first
        for header_name in &header_config.remove_request_headers {
            if let Ok(name) = HeaderName::from_bytes(header_name.as_bytes()) {
                headers.remove(&name);
                tracing::debug!(header_name = %header_name, "Removed request header");
            }
        }
        
        // Add/override headers
        for (name, value) in &header_config.request_headers {
            let header_name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|e| MiddlewareError::HeaderError(format!("Invalid header name '{}': {}", name, e)))?;
            let header_value = HeaderValue::from_str(value)
                .map_err(|e| MiddlewareError::HeaderError(format!("Invalid header value '{}': {}", value, e)))?;
            
            headers.insert(header_name, header_value);
            tracing::debug!(header_name = %name, header_value = %value, "Added request header");
        }
        
        // Override Host header if configured
        if let Some(host_override) = &header_config.override_host {
            headers.insert(
                "host",
                HeaderValue::from_str(host_override)
                    .map_err(|e| MiddlewareError::HeaderError(format!("Invalid host override: {}", e)))?
            );
            tracing::debug!(host = %host_override, "Overrode Host header");
        }
        
        Ok(req)
    }
    
    /// Apply header modifications to response
    fn apply_response_headers(
        &self,
        mut response: Response<Body>,
        header_config: &HeaderMiddlewareConfig,
    ) -> Result<Response<Body>, MiddlewareError> {
        let headers = response.headers_mut();
        
        // Remove headers first
        for header_name in &header_config.remove_response_headers {
            if let Ok(name) = HeaderName::from_bytes(header_name.as_bytes()) {
                headers.remove(&name);
                tracing::debug!(header_name = %header_name, "Removed response header");
            }
        }
        
        // Add/override headers
        for (name, value) in &header_config.response_headers {
            let header_name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|e| MiddlewareError::HeaderError(format!("Invalid header name '{}': {}", name, e)))?;
            let header_value = HeaderValue::from_str(value)
                .map_err(|e| MiddlewareError::HeaderError(format!("Invalid header value '{}': {}", value, e)))?;
            
            headers.insert(header_name, header_value);
            tracing::debug!(header_name = %name, header_value = %value, "Added response header");
        }
        
        Ok(response)
    }
    
    /// Transform request body based on configuration
    async fn transform_request_body(
        &self,
        req: Request<Body>,
        transform_config: &RequestTransformConfig,
    ) -> Result<Request<Body>, MiddlewareError> {
        // Extract the body
        let (parts, body) = req.into_parts();
        let body_bytes = axum::body::to_bytes(body, usize::MAX).await
            .map_err(|e| MiddlewareError::TransformError(format!("Failed to read request body: {}", e)))?;
        
        if body_bytes.is_empty() {
            // No body to transform
            return Ok(Request::from_parts(parts, Body::from(body_bytes)));
        }
        
        let mut body_string = String::from_utf8_lossy(&body_bytes).to_string();
        
        // Apply text replacements
        for replacement in &transform_config.replace_text {
            if replacement.regex_enabled {
                // Use regex replacement
                if let Ok(regex) = regex::Regex::new(&replacement.find) {
                    body_string = regex.replace_all(&body_string, &replacement.replace).to_string();
                    tracing::debug!(
                        pattern = %replacement.find,
                        replacement = %replacement.replace,
                        "Applied regex transformation to request body"
                    );
                }
            } else {
                // Simple string replacement
                body_string = body_string.replace(&replacement.find, &replacement.replace);
                tracing::debug!(
                    find = %replacement.find,
                    replace = %replacement.replace,
                    "Applied text replacement to request body"
                );
            }
        }
        
        // Apply JSON transformations if content-type is JSON
        if let Some(content_type) = parts.headers.get("content-type") {
            if let Ok(content_type_str) = content_type.to_str() {
                if content_type_str.contains("application/json") {
                    body_string = self.transform_json_body(
                        body_string,
                        &transform_config.add_json_fields,
                        &transform_config.remove_json_fields
                    )?;
                }
            }
        }
        
        let new_body = Body::from(body_string.into_bytes());
        Ok(Request::from_parts(parts, new_body))
    }
    
    /// Transform response body based on configuration
    async fn transform_response_body(
        &self,
        response: Response<Body>,
        transform_config: &ResponseTransformConfig,
    ) -> Result<Response<Body>, MiddlewareError> {
        let (parts, body) = response.into_parts();
        let body_bytes = axum::body::to_bytes(body, usize::MAX).await
            .map_err(|e| MiddlewareError::TransformError(format!("Failed to read response body: {}", e)))?;
        
        if body_bytes.is_empty() {
            // No body to transform
            return Ok(Response::from_parts(parts, Body::from(body_bytes)));
        }
        
        let mut body_string = String::from_utf8_lossy(&body_bytes).to_string();
        
        // Apply text replacements
        for replacement in &transform_config.replace_text {
            if replacement.regex_enabled {
                // Use regex replacement
                if let Ok(regex) = regex::Regex::new(&replacement.find) {
                    body_string = regex.replace_all(&body_string, &replacement.replace).to_string();
                    tracing::debug!(
                        pattern = %replacement.find,
                        replacement = %replacement.replace,
                        "Applied regex transformation to response body"
                    );
                }
            } else {
                // Simple string replacement
                body_string = body_string.replace(&replacement.find, &replacement.replace);
                tracing::debug!(
                    find = %replacement.find,
                    replace = %replacement.replace,
                    "Applied text replacement to response body"
                );
            }
        }
        
        // Apply JSON transformations if content-type is JSON
        if let Some(content_type) = parts.headers.get("content-type") {
            if let Ok(content_type_str) = content_type.to_str() {
                if content_type_str.contains("application/json") {
                    body_string = self.transform_json_body(
                        body_string,
                        &transform_config.add_json_fields,
                        &transform_config.remove_json_fields
                    )?;
                }
            }
        }
        
        let new_body = Body::from(body_string.into_bytes());
        Ok(Response::from_parts(parts, new_body))
    }
    
    /// Transform JSON body by adding/removing fields
    fn transform_json_body(
        &self,
        body_string: String,
        add_fields: &HashMap<String, Value>,
        remove_fields: &[String],
    ) -> Result<String, MiddlewareError> {
        if add_fields.is_empty() && remove_fields.is_empty() {
            return Ok(body_string);
        }
        
        let mut json: Value = serde_json::from_str(&body_string)
            .map_err(|e| MiddlewareError::TransformError(format!("Invalid JSON body: {}", e)))?;
        
        // Add fields
        if let Value::Object(ref mut map) = json {
            for (key, value) in add_fields {
                map.insert(key.clone(), value.clone());
                tracing::debug!(field = %key, "Added JSON field");
            }
            
            // Remove fields
            for key in remove_fields {
                if map.remove(key).is_some() {
                    tracing::debug!(field = %key, "Removed JSON field");
                }
            }
        }
        
        serde_json::to_string(&json)
            .map_err(|e| MiddlewareError::TransformError(format!("Failed to serialize JSON: {}", e)))
    }
    
    /// Apply compression to response
    async fn apply_compression(
        &self,
        response: Response<Body>,
        compression_config: &CompressionConfig,
    ) -> Result<Response<Body>, MiddlewareError> {
        let (parts, body) = response.into_parts();
        let body_bytes = axum::body::to_bytes(body, usize::MAX).await
            .map_err(|e| MiddlewareError::CompressionError(format!("Failed to read response body: {}", e)))?;
        
        // Check if response is large enough to compress
        if body_bytes.len() < compression_config.threshold_bytes {
            tracing::debug!(
                size = body_bytes.len(),
                threshold = compression_config.threshold_bytes,
                "Response too small for compression"
            );
            return Ok(Response::from_parts(parts, Body::from(body_bytes)));
        }
        
        // For now, implement basic gzip compression
        // In a real implementation, you would check Accept-Encoding header
        // and choose the best compression algorithm
        if compression_config.gzip {
            use flate2::{write::GzEncoder, Compression};
            use std::io::Write;
            
            let mut encoder = GzEncoder::new(Vec::new(), Compression::new(compression_config.level));
            encoder.write_all(&body_bytes)
                .map_err(|e| MiddlewareError::CompressionError(format!("Gzip compression failed: {}", e)))?;
            let compressed = encoder.finish()
                .map_err(|e| MiddlewareError::CompressionError(format!("Gzip finish failed: {}", e)))?;
            
            let mut new_parts = parts;
            new_parts.headers.insert("content-encoding", HeaderValue::from_static("gzip"));
            new_parts.headers.insert("content-length", HeaderValue::from_str(&compressed.len().to_string()).unwrap());
            
            tracing::debug!(
                original_size = body_bytes.len(),
                compressed_size = compressed.len(),
                ratio = format!("{:.2}%", (compressed.len() as f64 / body_bytes.len() as f64) * 100.0),
                "Applied gzip compression"
            );
            
            return Ok(Response::from_parts(new_parts, Body::from(compressed)));
        }
        
        // No compression applied
        Ok(Response::from_parts(parts, Body::from(body_bytes)))
    }
    
    /// Mark connection as finished (for rate limiting)
    pub fn finish_connection(&self, client_ip: &SocketAddr) {
        let client_id = client_ip.ip().to_string();
        let mut limiter = self.rate_limiter.lock().unwrap();
        
        if let Some(count) = limiter.active_connections.get_mut(&client_id) {
            if *count > 0 {
                *count -= 1;
            }
            if *count == 0 {
                limiter.active_connections.remove(&client_id);
            }
        }
    }
}

impl Default for MiddlewareProcessor {
    fn default() -> Self {
        Self::new()
    }
}
