// Phase 6.1 SSL/TLS Support - HTTPS Server Integration Tests

use httpserver_core::{Server, SslCertificateManager};
use axum::{Router, routing::get, response::Json};
use serde_json::json;
use tempfile::TempDir;
use tokio::time::Duration;
use rcgen::{Certificate, CertificateParams, DistinguishedName};
use std::fs;

#[cfg(test)]
mod https_integration_tests {
    use super::*;

    // Helper function to create test certificates
    fn create_test_certificate(domain: &str, temp_dir: &TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
        let cert_path = temp_dir.path().join(format!("{}.crt", domain.replace("*", "wildcard")));
        let key_path = temp_dir.path().join(format!("{}.key", domain.replace("*", "wildcard")));

        let mut params = CertificateParams::new(vec![domain.to_string()]);
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(rcgen::DnType::CommonName, domain);
        params.distinguished_name.push(rcgen::DnType::OrganizationName, "Test Organization");
        params.distinguished_name.push(rcgen::DnType::CountryName, "US");

        let cert = Certificate::from_params(params).expect("Failed to generate certificate");
        
        fs::write(&cert_path, cert.serialize_pem().expect("Failed to serialize certificate"))
            .expect("Failed to write certificate");
        fs::write(&key_path, cert.serialize_private_key_pem())
            .expect("Failed to write private key");

        (cert_path, key_path)
    }

    // Test handler for HTTPS server
    async fn test_handler() -> Json<serde_json::Value> {
        Json(json!({
            "message": "HTTPS server is working",
            "protocol": "https",
            "status": "ok"
        }))
    }

    #[tokio::test]
    async fn test_https_server_creation() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let (cert_path, key_path) = create_test_certificate("localhost", &temp_dir);
        
        let mut ssl_manager = SslCertificateManager::new();
        ssl_manager.load_certificate_from_files(
            "localhost".to_string(),
            &cert_path,
            &key_path,
            None,
        ).expect("Failed to load certificate");

        let ssl_config = ssl_manager.create_server_config("localhost")
            .expect("Failed to create SSL config");

        // Create HTTPS server
        let server = Server::new_with_ssl(8080, ssl_config, 8443);
        assert!(server.ssl_config.is_some());
        assert_eq!(server.https_port, Some(8443));
    }

    #[tokio::test]
    async fn test_https_server_startup() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let (cert_path, key_path) = create_test_certificate("localhost", &temp_dir);
        
        let mut ssl_manager = SslCertificateManager::new();
        ssl_manager.load_certificate_from_files(
            "localhost".to_string(),
            &cert_path,
            &key_path,
            None,
        ).expect("Failed to load certificate");

        let ssl_config = ssl_manager.create_server_config("localhost")
            .expect("Failed to create SSL config");

        // Create test router
        let app = Router::new()
            .route("/test", get(test_handler));

        // Create HTTPS server
        let server = Server::new_with_ssl(8081, ssl_config, 8444);        // Start server in background with timeout
        let server_task = tokio::spawn(async move {
            let _ = server.start(app).await;
        });

        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Test that server task is running (not completed immediately)
        assert!(!server_task.is_finished());

        // Cleanup - abort the server task
        server_task.abort();
        let _ = server_task.await;
    }

    #[tokio::test]
    async fn test_wildcard_certificate_support() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let (cert_path, key_path) = create_test_certificate("*.httpserver.io", &temp_dir);
        
        let mut ssl_manager = SslCertificateManager::new();
        ssl_manager.load_certificate_from_files(
            "*.httpserver.io".to_string(),
            &cert_path,
            &key_path,
            None,
        ).expect("Failed to load wildcard certificate");

        // Test that wildcard domain is detected
        assert_eq!(ssl_manager.get_wildcard_domain(), Some("*.httpserver.io".to_string()));

        // Test SNI certificate selection
        assert!(ssl_manager.get_certificate_for_sni("api.httpserver.io").is_some());        assert!(ssl_manager.get_certificate_for_sni("web.httpserver.io").is_some());
        assert!(ssl_manager.get_certificate_for_sni("httpserver.io").is_none()); // Base domain doesn't match wildcard
        assert!(ssl_manager.get_certificate_for_sni("sub.api.httpserver.io").is_none()); // Too many subdomains

        let _ssl_config = ssl_manager.create_server_config("api.httpserver.io")
            .expect("Failed to create SSL config for wildcard subdomain");

        // Verify SSL config was created successfully
        // SSL config should have been created with valid certificate data
    }

    #[tokio::test]
    async fn test_multiple_certificates() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let (cert_path1, key_path1) = create_test_certificate("example.com", &temp_dir);
        let (cert_path2, key_path2) = create_test_certificate("*.httpserver.io", &temp_dir);
        
        let mut ssl_manager = SslCertificateManager::new();
        
        // Load multiple certificates
        ssl_manager.load_certificate_from_files(
            "example.com".to_string(),
            &cert_path1,
            &key_path1,
            None,
        ).expect("Failed to load first certificate");

        ssl_manager.load_certificate_from_files(
            "*.httpserver.io".to_string(),
            &cert_path2,
            &key_path2,
            None,
        ).expect("Failed to load second certificate");

        // Test certificate selection
        assert!(ssl_manager.get_certificate_for_sni("example.com").is_some());
        assert!(ssl_manager.get_certificate_for_sni("api.httpserver.io").is_some());
        assert!(ssl_manager.get_certificate_for_sni("unknown.domain").is_none());

        // Test that we can create SSL configs for different domains
        assert!(ssl_manager.create_server_config("example.com").is_ok());
        assert!(ssl_manager.create_server_config("api.httpserver.io").is_ok());
        assert!(ssl_manager.create_server_config("unknown.domain").is_err());
    }

    #[tokio::test]
    async fn test_ssl_config_error_handling() {
        let ssl_manager = SslCertificateManager::new();

        // Test creating SSL config with no certificates
        let result = ssl_manager.create_server_config("example.com");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No SSL certificate found"));
    }
}
