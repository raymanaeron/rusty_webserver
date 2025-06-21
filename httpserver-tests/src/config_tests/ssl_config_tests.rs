// SSL configuration loading and verification tests
use httpserver_config::Config;
use std::path::PathBuf;
use tempfile::TempDir;
use std::fs;

/// Create a temporary directory with test files
#[allow(dead_code)]
fn create_test_directory() -> TempDir {
    let temp_dir = TempDir::new().unwrap();

    // Create a test index.html file
    let index_path = temp_dir.path().join("index.html");
    fs::write(&index_path, "<html><body>Test</body></html>").unwrap();

    temp_dir
}

/// Create a test configuration file
#[allow(dead_code)]
fn create_test_config_file(temp_dir: &TempDir, content: &str) -> PathBuf {
    let config_path = temp_dir.path().join("ssl_test_config.toml");
    fs::write(&config_path, content).unwrap();
    config_path
}

#[test]
fn test_ssl_config_loading_basic() {
    let temp_dir = create_test_directory();
    let toml_content = format!(
        r#"
[static_config]
directory = "{}"

[server.ssl]
enabled = true
https_port = 8443
cert_file = "certs/server.crt"
key_file = "certs/server.key"
cert_chain_file = "certs/chain.pem"
force_https = true

[[proxy]]
path = "/api/*"
targets = [{{ url = "http://localhost:3000", weight = 1 }}]
"#,
        temp_dir.path().to_string_lossy().replace('\\', "/")
    );

    let config_path = create_test_config_file(&temp_dir, &toml_content);
    let config = Config::load_from_file(&config_path).unwrap();

    // Verify SSL configuration is loaded correctly
    assert!(config.server.ssl.is_some());
    let ssl_config = config.server.ssl.unwrap();
    
    assert!(ssl_config.enabled);
    assert_eq!(ssl_config.https_port, 8443);
    assert_eq!(ssl_config.cert_file, Some(PathBuf::from("certs/server.crt")));
    assert_eq!(ssl_config.key_file, Some(PathBuf::from("certs/server.key")));
    assert_eq!(ssl_config.cert_chain_file, Some(PathBuf::from("certs/chain.pem")));
    assert!(ssl_config.force_https);
}

#[test]
fn test_ssl_config_with_redirect_section() {
    let temp_dir = create_test_directory();
    let toml_content = format!(
        r#"
[static_config]
directory = "{}"

[server.ssl]
enabled = true
https_port = 8443
cert_file = "certs/server.crt"
key_file = "certs/server.key"

[server.ssl.redirect]
enabled = true
exempt_paths = ["/health", "/ping", "/metrics"]

[[proxy]]
path = "/api/*"
targets = [{{ url = "http://localhost:3000", weight = 1 }}]
"#,
        temp_dir.path().to_string_lossy().replace('\\', "/")
    );

    let config_path = create_test_config_file(&temp_dir, &toml_content);
    let config = Config::load_from_file(&config_path).unwrap();

    // Verify SSL configuration is loaded correctly
    assert!(config.server.ssl.is_some());
    let ssl_config = config.server.ssl.unwrap();
    
    assert!(ssl_config.enabled);
    assert_eq!(ssl_config.https_port, 8443);
    
    // Verify redirect configuration
    assert!(ssl_config.redirect.is_some());
    let redirect_config = ssl_config.redirect.unwrap();
    assert!(redirect_config.enabled);
    assert_eq!(redirect_config.exempt_paths, vec!["/health", "/ping", "/metrics"]);
}

#[test]
fn test_ssl_config_with_wildcard() {
    let temp_dir = create_test_directory();
    let toml_content = format!(
        r#"
[static_config]
directory = "{}"

[server.ssl]
enabled = true
https_port = 443

[server.ssl.wildcard]
enabled = true
domain = "*.httpserver.io"
cert_file = "certs/wildcard.crt"
key_file = "certs/wildcard.key"

[[proxy]]
path = "/api/*"
targets = [{{ url = "http://localhost:3000", weight = 1 }}]
"#,
        temp_dir.path().to_string_lossy().replace('\\', "/")
    );

    let config_path = create_test_config_file(&temp_dir, &toml_content);
    let config = Config::load_from_file(&config_path).unwrap();

    // Verify SSL configuration is loaded correctly
    assert!(config.server.ssl.is_some());
    let ssl_config = config.server.ssl.unwrap();
    
    assert!(ssl_config.enabled);
    assert_eq!(ssl_config.https_port, 443);
    
    // Verify wildcard configuration
    assert!(ssl_config.wildcard.is_some());
    let wildcard_config = ssl_config.wildcard.unwrap();
    assert_eq!(wildcard_config.domain, "*.httpserver.io");
    assert_eq!(wildcard_config.cert_file, PathBuf::from("certs/wildcard.crt"));
    assert_eq!(wildcard_config.key_file, PathBuf::from("certs/wildcard.key"));
}

#[test]
fn test_ssl_config_with_letsencrypt() {
    let temp_dir = create_test_directory();
    let toml_content = format!(
        r#"
[static_config]
directory = "{}"

[server.ssl]
enabled = true

[server.ssl.lets_encrypt]
enabled = true
email = "admin@httpserver.io"
domain = "httpserver.io"
staging = false

[server.ssl.lets_encrypt.dns_challenge]
provider = "cloudflare"
timeout_seconds = 300
credentials = {{ api_token = "test-token-123" }}

[[proxy]]
path = "/api/*"
targets = [{{ url = "http://localhost:3000", weight = 1 }}]
"#,
        temp_dir.path().to_string_lossy().replace('\\', "/")
    );    let config_path = create_test_config_file(&temp_dir, &toml_content);
    let config = Config::load_from_file(&config_path).unwrap();

    // Verify SSL configuration is loaded correctly
    assert!(config.server.ssl.is_some());
    let ssl_config = config.server.ssl.unwrap();
    
    assert!(ssl_config.enabled);    // Verify Let's Encrypt configuration
    assert!(ssl_config.lets_encrypt.is_some());
    let le_config = ssl_config.lets_encrypt.unwrap();
    assert!(le_config.enabled);
    assert_eq!(le_config.email, "admin@httpserver.io");
    assert_eq!(le_config.domain, Some("httpserver.io".to_string()));
    assert!(!le_config.staging);
    
    // Verify DNS challenge configuration
    assert!(le_config.dns_challenge.is_some());
    let dns_config = le_config.dns_challenge.unwrap();
    assert_eq!(dns_config.provider, "cloudflare");
    assert_eq!(dns_config.timeout_seconds, 300);
    assert_eq!(dns_config.credentials.get("api_token"), Some(&"test-token-123".to_string()));
}

#[test]
fn test_ssl_config_with_route_ssl() {
    let temp_dir = create_test_directory();
    let toml_content = format!(
        r#"
[static_config]
directory = "{}"

[server.ssl]
enabled = true

[[proxy]]
path = "/api/*"
targets = [{{ url = "http://localhost:3000", weight = 1 }}]

[proxy.ssl]
enabled = true
verify_backend_ssl = true
cert_file = "certs/client.crt"
key_file = "certs/client.key"
backend_ssl = true
"#,
        temp_dir.path().to_string_lossy().replace('\\', "/")
    );

    let config_path = create_test_config_file(&temp_dir, &toml_content);
    let config = Config::load_from_file(&config_path).unwrap();

    // Verify route SSL configuration
    assert_eq!(config.proxy.len(), 1);
    let route = &config.proxy[0];
    
    assert!(route.ssl.is_some());
    let route_ssl = route.ssl.as_ref().unwrap();
    assert!(route_ssl.enabled);
    assert!(route_ssl.verify_backend_ssl);
    assert!(route_ssl.backend_ssl);
    assert_eq!(route_ssl.cert_file, Some(PathBuf::from("certs/client.crt")));
    assert_eq!(route_ssl.key_file, Some(PathBuf::from("certs/client.key")));
}

#[test]
fn test_ssl_config_defaults() {
    let temp_dir = create_test_directory();
    let toml_content = format!(
        r#"
[static_config]
directory = "{}"

[server.ssl]
enabled = true

[[proxy]]
path = "/api/*"
targets = [{{ url = "http://localhost:3000", weight = 1 }}]
"#,
        temp_dir.path().to_string_lossy().replace('\\', "/")
    );

    let config_path = create_test_config_file(&temp_dir, &toml_content);
    let config = Config::load_from_file(&config_path).unwrap();

    // Verify SSL configuration defaults
    assert!(config.server.ssl.is_some());
    let ssl_config = config.server.ssl.unwrap();
    
    assert!(ssl_config.enabled);
    assert_eq!(ssl_config.https_port, 443); // Default value
    assert!(!ssl_config.force_https); // Default value
    assert_eq!(ssl_config.protocols, vec!["TLSv1.2", "TLSv1.3"]); // Default values
    assert!(ssl_config.cipher_suites.is_empty()); // Default empty
    
    // Verify optional fields are None by default
    assert!(ssl_config.cert_file.is_none());
    assert!(ssl_config.key_file.is_none());
    assert!(ssl_config.cert_chain_file.is_none());
    assert!(ssl_config.wildcard.is_none());
    assert!(ssl_config.lets_encrypt.is_none());
    assert!(ssl_config.redirect.is_none());
}
