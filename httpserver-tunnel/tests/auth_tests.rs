//! Authentication Module Tests
//! Tests for API key, token, and certificate authentication

use httpserver_tunnel::auth::TunnelAuthenticator;
use httpserver_tunnel::config::{TunnelAuthConfig, TokenRefreshConfig};
use std::collections::HashMap;

#[tokio::test]
async fn test_api_key_authentication() {
    let config = TunnelAuthConfig {
        method: "api_key".to_string(),
        api_key: Some("test-api-key-12345".to_string()),
        token: None,
        cert_file: None,
        key_file: None,
        user: None,
        headers: HashMap::new(),
        token_refresh: TokenRefreshConfig::default(),
    };

    let authenticator = TunnelAuthenticator::new(config).unwrap();
    let credentials = authenticator.get_credentials().await.unwrap();

    assert_eq!(credentials.auth_method, "api_key");
    assert!(credentials.headers.contains_key("Authorization"));
    assert_eq!(
        credentials.headers.get("Authorization"),
        Some(&"Bearer test-api-key-12345".to_string())
    );
    assert!(credentials.headers.contains_key("User-Agent"));
}

#[tokio::test]
async fn test_token_authentication() {
    let config = TunnelAuthConfig {
        method: "token".to_string(),
        api_key: None,
        token: Some("test-token-67890".to_string()),
        cert_file: None,
        key_file: None,
        user: None,
        headers: HashMap::new(),
        token_refresh: TokenRefreshConfig::default(),
    };

    let authenticator = TunnelAuthenticator::new(config).unwrap();
    let credentials = authenticator.get_credentials().await.unwrap();

    assert_eq!(credentials.auth_method, "token");
    assert!(credentials.headers.contains_key("Authorization"));
    assert_eq!(
        credentials.headers.get("Authorization"),
        Some(&"Bearer test-token-67890".to_string())
    );
}

#[tokio::test]
async fn test_custom_headers() {
    let mut custom_headers = HashMap::new();
    custom_headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());
    custom_headers.insert("X-Client-Version".to_string(), "1.0.0".to_string());

    let config = TunnelAuthConfig {
        method: "api_key".to_string(),
        api_key: Some("test-key".to_string()),
        token: None,
        cert_file: None,
        key_file: None,
        user: Some("test-user".to_string()),
        headers: custom_headers,
        token_refresh: TokenRefreshConfig::default(),
    };

    let authenticator = TunnelAuthenticator::new(config).unwrap();
    let credentials = authenticator.get_credentials().await.unwrap();

    assert!(credentials.headers.contains_key("X-Custom-Header"));
    assert_eq!(
        credentials.headers.get("X-Custom-Header"),
        Some(&"custom-value".to_string())
    );
    assert!(credentials.headers.contains_key("X-Tunnel-User"));
    assert_eq!(
        credentials.headers.get("X-Tunnel-User"),
        Some(&"test-user".to_string())
    );
}

#[tokio::test]
async fn test_missing_api_key() {
    let config = TunnelAuthConfig {
        method: "api_key".to_string(),
        api_key: None, // Missing API key
        token: None,
        cert_file: None,
        key_file: None,
        user: None,
        headers: HashMap::new(),
        token_refresh: TokenRefreshConfig::default(),
    };

    let authenticator = TunnelAuthenticator::new(config).unwrap();
    let result = authenticator.get_credentials().await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("API key not configured"));
}

#[tokio::test]
async fn test_unsupported_auth_method() {
    let config = TunnelAuthConfig {
        method: "unsupported_method".to_string(),
        api_key: None,
        token: None,
        cert_file: None,
        key_file: None,
        user: None,
        headers: HashMap::new(),
        token_refresh: TokenRefreshConfig::default(),
    };

    let authenticator = TunnelAuthenticator::new(config).unwrap();
    let result = authenticator.get_credentials().await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Unsupported auth method"));
}

#[tokio::test]
async fn test_certificate_authentication() {
    let config = TunnelAuthConfig {
        method: "certificate".to_string(),
        api_key: None,
        token: None,
        cert_file: Some("client.pem".into()),
        key_file: Some("key.pem".into()),
        user: Some("cert-user".to_string()),
        headers: HashMap::new(),
        token_refresh: TokenRefreshConfig::default(),
    };

    let authenticator = TunnelAuthenticator::new(config).unwrap();
    let result = authenticator.get_credentials().await;

    // Certificate auth should succeed (file validation happens at TLS level)
    assert!(result.is_ok());
    
    let credentials = result.unwrap();
    assert_eq!(credentials.auth_method, "certificate");
    assert!(credentials.headers.contains_key("X-Auth-Method"));
    assert_eq!(
        credentials.headers.get("X-Auth-Method"),
        Some(&"certificate".to_string())
    );
    assert!(credentials.headers.contains_key("X-Tunnel-User"));
    assert_eq!(
        credentials.headers.get("X-Tunnel-User"),
        Some(&"cert-user".to_string())
    );
}
