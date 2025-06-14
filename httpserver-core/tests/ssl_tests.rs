// Phase 6.1 SSL/TLS Support Tests
// SSL Foundation - Certificate Loading and Management Tests

use httpserver_core::SslCertificateManager;
use std::path::PathBuf;
use std::fs;
use tempfile::TempDir;
use rcgen::{ Certificate, CertificateParams, DistinguishedName };

#[cfg(test)]
mod ssl_tests {
    use super::*; // Helper function to create temporary certificate files for testing
    fn create_test_cert_files(temp_dir: &TempDir) -> (PathBuf, PathBuf) {
        let cert_path = temp_dir.path().join("test.crt");
        let key_path = temp_dir.path().join("test.key");

        // Generate a self-signed certificate using rcgen
        let mut params = CertificateParams::new(vec!["test.dev".to_string()]);
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(rcgen::DnType::CommonName, "test.dev");
        params.distinguished_name.push(rcgen::DnType::OrganizationName, "Test Organization");
        params.distinguished_name.push(rcgen::DnType::CountryName, "US");

        let cert = Certificate::from_params(params).expect("Failed to generate certificate");

        // Write certificate and private key to files
        fs::write(
            &cert_path,
            cert.serialize_pem().expect("Failed to serialize certificate")
        ).expect("Failed to write test certificate");
        fs::write(&key_path, cert.serialize_private_key_pem()).expect("Failed to write test key");

        (cert_path, key_path)
    }

    // Helper function to create wildcard certificate files
    fn create_wildcard_cert_files(temp_dir: &TempDir) -> (PathBuf, PathBuf) {
        let cert_path = temp_dir.path().join("wildcard.crt");
        let key_path = temp_dir.path().join("wildcard.key");

        // Generate a wildcard certificate using rcgen
        let mut params = CertificateParams::new(vec!["*.httpserver.io".to_string()]);
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(rcgen::DnType::CommonName, "*.httpserver.io");
        params.distinguished_name.push(rcgen::DnType::OrganizationName, "HTTPServer Test");
        params.distinguished_name.push(rcgen::DnType::CountryName, "US");

        let cert = Certificate::from_params(params).expect(
            "Failed to generate wildcard certificate"
        );

        // Write certificate and private key to files
        fs::write(
            &cert_path,
            cert.serialize_pem().expect("Failed to serialize wildcard certificate")
        ).expect("Failed to write wildcard certificate");
        fs::write(&key_path, cert.serialize_private_key_pem()).expect(
            "Failed to write wildcard key"
        );

        (cert_path, key_path)
    }

    #[test]
    fn test_ssl_certificate_manager_creation() {
        let manager = SslCertificateManager::new();
        assert!(!manager.has_certificates());
        assert!(manager.get_wildcard_domain().is_none());
    }

    #[test]
    fn test_ssl_certificate_loading() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let (cert_path, key_path) = create_test_cert_files(&temp_dir);

        let mut manager = SslCertificateManager::new();
        let result = manager.load_certificate_from_files(
            "test.dev".to_string(),
            &cert_path,
            &key_path,
            None
        );

        assert!(result.is_ok(), "Certificate loading should succeed");
        assert!(manager.has_certificates());
    }
    #[test]
    fn test_wildcard_certificate_detection() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let (cert_path, key_path) = create_wildcard_cert_files(&temp_dir);

        let mut manager = SslCertificateManager::new();
        let result = manager.load_certificate_from_files(
            "*.httpserver.io".to_string(),
            &cert_path,
            &key_path,
            None
        );

        assert!(result.is_ok(), "Wildcard certificate loading should succeed");
        assert!(manager.has_certificates());
        assert_eq!(manager.get_wildcard_domain(), Some("*.httpserver.io".to_string()));
    }

    #[test]
    fn test_wildcard_domain_matching() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let (cert_path, key_path) = create_wildcard_cert_files(&temp_dir);

        let mut manager = SslCertificateManager::new();
        manager
            .load_certificate_from_files(
                "*.httpserver.io".to_string(),
                &cert_path,
                &key_path,
                None::<&PathBuf>
            )
            .expect("Failed to load wildcard certificate");

        // Test wildcard matching logic
        assert!(manager.has_certificates());
        assert_eq!(manager.get_wildcard_domain(), Some("*.httpserver.io".to_string()));
    }

    #[test]
    fn test_multiple_certificates() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let (cert_path1, key_path1) = create_test_cert_files(&temp_dir);

        // Create second certificate files
        let cert_path2 = temp_dir.path().join("test2.crt");
        let key_path2 = temp_dir.path().join("test2.key");

        // Reuse the same test cert content for simplicity
        fs::copy(&cert_path1, &cert_path2).expect("Failed to copy cert");
        fs::copy(&key_path1, &key_path2).expect("Failed to copy key");

        let mut manager = SslCertificateManager::new(); // Load first certificate
        manager
            .load_certificate_from_files("test1.dev".to_string(), &cert_path1, &key_path1, None)
            .expect("Failed to load first certificate");

        // Load second certificate
        manager
            .load_certificate_from_files("test2.dev".to_string(), &cert_path2, &key_path2, None)
            .expect("Failed to load second certificate");

        assert!(manager.has_certificates());
    }

    #[test]
    fn test_ssl_server_config_creation() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let (cert_path, key_path) = create_test_cert_files(&temp_dir);

        let mut manager = SslCertificateManager::new();
        manager
            .load_certificate_from_files("test.dev".to_string(), &cert_path, &key_path, None)
            .expect("Failed to load certificate");

        let result = manager.create_server_config("test.dev");
        assert!(result.is_ok(), "SSL server config creation should succeed");
    }

    #[test]
    fn test_invalid_certificate_files() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let invalid_cert_path = temp_dir.path().join("invalid.crt");
        let invalid_key_path = temp_dir.path().join("invalid.key");

        // Write invalid certificate content
        fs::write(&invalid_cert_path, "invalid certificate content").expect(
            "Failed to write invalid cert"
        );
        fs::write(&invalid_key_path, "invalid key content").expect("Failed to write invalid key");

        let mut manager = SslCertificateManager::new();
        let result = manager.load_certificate_from_files(
            "test.dev".to_string(),
            &invalid_cert_path,
            &invalid_key_path,
            None
        );

        assert!(result.is_err(), "Loading invalid certificates should fail");
    }

    #[test]
    fn test_nonexistent_certificate_files() {
        let mut manager = SslCertificateManager::new();
        let result = manager.load_certificate_from_files(
            "test.dev".to_string(),
            "/nonexistent/cert.crt",
            "/nonexistent/key.key",
            None
        );

        assert!(result.is_err(), "Loading nonexistent certificates should fail");
    }

    #[test]
    fn test_domain_matching_logic() {
        use super::SslCertificateManager;
        
        // Test exact domain matching
        assert!(SslCertificateManager::matches_wildcard_domain("example.com", "example.com"));
        assert!(!SslCertificateManager::matches_wildcard_domain("test.example.com", "example.com"));
        
        // Test wildcard matching
        assert!(SslCertificateManager::matches_wildcard_domain("api.httpserver.io", "*.httpserver.io"));
        assert!(SslCertificateManager::matches_wildcard_domain("web.httpserver.io", "*.httpserver.io"));
        assert!(!SslCertificateManager::matches_wildcard_domain("httpserver.io", "*.httpserver.io"));
        assert!(!SslCertificateManager::matches_wildcard_domain("sub.api.httpserver.io", "*.httpserver.io"));
    }

    #[test]
    fn test_sni_certificate_selection() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let (cert_path1, key_path1) = create_test_cert_files(&temp_dir);
        let (cert_path2, key_path2) = create_wildcard_cert_files(&temp_dir);
        
        let mut manager = SslCertificateManager::new();
        
        // Load exact domain certificate
        manager.load_certificate_from_files(
            "api.example.com".to_string(),
            &cert_path1,
            &key_path1,
            None,
        ).expect("Failed to load exact domain certificate");
        
        // Load wildcard certificate
        manager.load_certificate_from_files(
            "*.httpserver.io".to_string(),
            &cert_path2,
            &key_path2,
            None,
        ).expect("Failed to load wildcard certificate");

        // Test exact match takes precedence
        assert!(manager.get_certificate_for_sni("api.example.com").is_some());
        
        // Test wildcard matching
        assert!(manager.get_certificate_for_sni("web.httpserver.io").is_some());
        assert!(manager.get_certificate_for_sni("api.httpserver.io").is_some());
        
        // Test no match
        assert!(manager.get_certificate_for_sni("example.org").is_none());
        assert!(manager.get_certificate_for_sni("sub.web.httpserver.io").is_none());
    }

    #[test]
    fn test_ssl_redirect_config() {
        use httpserver_core::SslRedirectConfig;
        
        let config = SslRedirectConfig::new(true, 443);
        assert!(config.enabled);
        assert_eq!(config.https_port, 443);
        assert!(config.is_exempt("/health"));
        assert!(config.is_exempt("/ping"));
        assert!(!config.is_exempt("/api/users"));
        
        // Test with custom HTTPS port
        let custom_config = SslRedirectConfig::new(false, 8443);
        assert!(!custom_config.enabled);
        assert_eq!(custom_config.https_port, 8443);
          // Test exempt path matching
        assert!(custom_config.is_exempt("/health/check"));
        assert!(custom_config.is_exempt("/ping/status"));
        assert!(!custom_config.is_exempt("/api/users")); // Should not match
        assert!(custom_config.is_exempt("/healthz")); // This SHOULD match as it starts with /health
    }
}
