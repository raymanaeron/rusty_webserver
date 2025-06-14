// SSL/TLS termination and certificate management
use std::fs;
use std::path::Path;
use std::sync::Arc;
use rustls::{ Certificate, PrivateKey, ServerConfig };
use rustls_pemfile::{ certs, pkcs8_private_keys, rsa_private_keys };
use tracing;

/// SSL certificate and key pair
#[derive(Debug, Clone)]
pub struct SslCertificate {
    pub cert_chain: Vec<Certificate>,
    pub private_key: PrivateKey,
}

/// SSL certificate manager for loading and managing certificates
pub struct SslCertificateManager {
    certificates: std::collections::HashMap<String, SslCertificate>,
    wildcard_cert: Option<SslCertificate>,
}

impl SslCertificateManager {
    /// Create a new SSL certificate manager
    pub fn new() -> Self {
        Self {
            certificates: std::collections::HashMap::new(),
            wildcard_cert: None,
        }
    }

    /// Load certificate and private key from PEM files
    pub fn load_certificate_from_files<P: AsRef<Path>>(
        &mut self,
        domain: String,
        cert_file: P,
        key_file: P,
        cert_chain_file: Option<P>
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cert_path = cert_file.as_ref();
        let key_path = key_file.as_ref();

        tracing::info!(
            domain = %domain,
            cert_file = %cert_path.display(),
            key_file = %key_path.display(),
            "Loading SSL certificate"
        );

        // Load certificate file
        let cert_file_content = fs
            ::read(cert_path)
            .map_err(|e|
                format!("Failed to read certificate file '{}': {}", cert_path.display(), e)
            )?;

        let mut cert_reader = std::io::Cursor::new(cert_file_content);
        let cert_chain = certs(&mut cert_reader)
            .map_err(|e|
                format!("Failed to parse certificate file '{}': {}", cert_path.display(), e)
            )?
            .into_iter()
            .map(Certificate)
            .collect::<Vec<_>>();

        if cert_chain.is_empty() {
            return Err(format!("No certificates found in file '{}'", cert_path.display()).into());
        }

        // Load additional certificates from chain file if provided
        let mut final_cert_chain = cert_chain;
        if let Some(chain_path) = cert_chain_file {
            let chain_path = chain_path.as_ref();
            tracing::info!(
                chain_file = %chain_path.display(),
                "Loading certificate chain"
            );

            let chain_file_content = fs
                ::read(chain_path)
                .map_err(|e|
                    format!(
                        "Failed to read certificate chain file '{}': {}",
                        chain_path.display(),
                        e
                    )
                )?;

            let mut chain_reader = std::io::Cursor::new(chain_file_content);
            let chain_certs = certs(&mut chain_reader)
                .map_err(|e|
                    format!(
                        "Failed to parse certificate chain file '{}': {}",
                        chain_path.display(),
                        e
                    )
                )?
                .into_iter()
                .map(Certificate)
                .collect::<Vec<_>>();

            final_cert_chain.extend(chain_certs);
        }

        // Load private key file
        let key_file_content = fs
            ::read(key_path)
            .map_err(|e|
                format!("Failed to read private key file '{}': {}", key_path.display(), e)
            )?;
        let mut key_reader = std::io::Cursor::new(&key_file_content);

        // Try PKCS#8 format first, then RSA format
        let private_key = match pkcs8_private_keys(&mut key_reader) {
            Ok(mut keys) if !keys.is_empty() => {
                if keys.len() > 1 {
                    tracing::warn!("Multiple private keys found, using the first one");
                }
                PrivateKey(keys.remove(0))
            }
            _ => {
                // Reset reader and try RSA format
                let mut key_reader = std::io::Cursor::new(&key_file_content);
                match rsa_private_keys(&mut key_reader) {
                    Ok(mut keys) if !keys.is_empty() => {
                        if keys.len() > 1 {
                            tracing::warn!("Multiple RSA private keys found, using the first one");
                        }
                        PrivateKey(keys.remove(0))
                    }
                    _ => {
                        return Err(
                            format!(
                                "No valid private key found in file '{}'",
                                key_path.display()
                            ).into()
                        );
                    }
                }
            }
        };
        let ssl_cert = SslCertificate {
            cert_chain: final_cert_chain.clone(),
            private_key,
        };

        // Check if this is a wildcard certificate
        if domain.starts_with("*.") {
            tracing::info!(domain = %domain, "Loaded wildcard certificate");
            self.wildcard_cert = Some(ssl_cert.clone());
        }

        let cert_count = final_cert_chain.len();
        self.certificates.insert(domain.clone(), ssl_cert);

        tracing::info!(
            domain = %domain,
            cert_count = cert_count,
            "SSL certificate loaded successfully"
        );

        Ok(())
    }

    /// Get certificate for a specific domain
    pub fn get_certificate(&self, domain: &str) -> Option<&SslCertificate> {
        // First try exact domain match
        if let Some(cert) = self.certificates.get(domain) {
            return Some(cert);
        }

        // Then try wildcard certificate for subdomains
        if let Some(wildcard_cert) = &self.wildcard_cert {
            // Check if this domain could be covered by wildcard
            for wildcard_domain in self.certificates.keys() {
                if wildcard_domain.starts_with("*.") {
                    let base_domain = &wildcard_domain[2..]; // Remove "*."
                    if domain.ends_with(base_domain) && domain != base_domain {
                        tracing::debug!(
                            domain = %domain,
                            wildcard_domain = %wildcard_domain,
                            "Using wildcard certificate"
                        );
                        return Some(wildcard_cert);
                    }
                }
            }
        }

        None
    }

    /// Create rustls ServerConfig for SSL termination
    pub fn create_server_config(
        &self,
        domain: &str
    ) -> Result<Arc<ServerConfig>, Box<dyn std::error::Error>> {
        let ssl_cert = self
            .get_certificate(domain)
            .ok_or_else(|| format!("No SSL certificate found for domain '{}'", domain))?;

        let config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(ssl_cert.cert_chain.clone(), ssl_cert.private_key.clone())
            .map_err(|e| format!("Failed to create SSL server config: {}", e))?;

        tracing::info!(
            domain = %domain,
            "SSL server config created successfully"
        );

        Ok(Arc::new(config))
    }

    /// List all loaded certificates
    pub fn list_certificates(&self) -> Vec<String> {
        self.certificates.keys().cloned().collect()
    }

    /// Check if any certificates are loaded
    pub fn has_certificates(&self) -> bool {
        !self.certificates.is_empty()
    }

    /// Get wildcard certificate domain if available
    pub fn get_wildcard_domain(&self) -> Option<String> {
        self.certificates
            .keys()
            .find(|domain| domain.starts_with("*."))
            .cloned()
    }
}

/// SSL redirect middleware for forcing HTTPS
pub struct SslRedirectConfig {
    pub enabled: bool,
    pub https_port: u16,
    pub exempt_paths: Vec<String>, // Paths that don't require HTTPS (e.g., health checks)
}

impl SslRedirectConfig {
    pub fn new(enabled: bool, https_port: u16) -> Self {
        Self {
            enabled,
            https_port,
            exempt_paths: vec!["/health".to_string(), "/ping".to_string()],
        }
    }

    /// Check if a path is exempt from HTTPS redirect
    pub fn is_exempt(&self, path: &str) -> bool {
        self.exempt_paths.iter().any(|exempt_path| path.starts_with(exempt_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_cert_files() -> Result<
        (NamedTempFile, NamedTempFile),
        Box<dyn std::error::Error>
    > {
        // Create a simple self-signed certificate for testing
        let cert_pem =
            r#"-----BEGIN CERTIFICATE-----
MIIBhTCCAS6gAwIBAgIJAKPCb4K4F+j6MA0GCSqGSIb3DQEBCwUAMBkxFzAVBgNV
BAMTDnRlc3QuZXhhbXBsZS5jb20wHhcNMjQwMTAxMDAwMDAwWhcNMjUwMTAxMDAw
MDAwWjAZMRcwFQYDVQQDEw50ZXN0LmV4YW1wbGUuY29tMFwwDQYJKoZIhvcNAQEB
BQADSwAwSAJBALB8dL8J6/2W5/dL3/2B5/dL5/dL6/2W8/dL7/2B9/dL0/dL1/2W
2/dL3/2B4/dL5/dL6/2W8/dL7/2B9/dL0/dL1/2W2CAwEAAaMkMCIwCQYDVR0TBAIw
ADAVBgNVHREEDjAMggp0ZXN0LmRvbWFpbDANBgkqhkiG9w0BAQsFAANBAIZ/1/2W
3/dL4/2B5/dL6/2W7/dL8/2B9/dL0/dL1/2W2/dL3/2B4/dL5/dL6/2W8/dL7/2B
9/dL0/dL1/2W2=
-----END CERTIFICATE-----"#;

        let key_pem =
            r#"-----BEGIN PRIVATE KEY-----
MIIBVAIBADANBgkqhkiG9w0BAQEFAASCAT4wggE6AgEAAkEAsHx0vwnr/Zbn90vf
/YHn90vn90vr/Zbz90vv/YH390vT90vX/Zbb90vf/YHj90vn90vr/Zbz90vv/YH3
90vT90vX/ZbYIDAQABAkAoJ1/2W4/dL5/2B6/dL7/2W8/dL9/2B0/dL1/2W2/dL
3/2B4/dL5/2W6/dL7/2B8/dL9/2B0/dL1/2W2/dL3/2B4AhEAzL5/2W6/dL7/2B
8/dL9/2B0/dL1/2W2/dL3/2B4/dL5/2W6/dL7/2B8wIRAM6/dL7/2B8/dL9/2B
0/dL1/2W2/dL3/2B4/dL5/2W6/dL7/2B8CEB3/2B4/dL5/2W6/dL7/2B8/dL9/2B
0/dL1/2W2/dL3/2B4/dL5wIQC/dL5/2W6/dL7/2B8/dL9/2B0/dL1/2W2/dL3/2B
4/dL5/2W6/dL7wIQA/dL7/2B8/dL9/2B0/dL1/2W2/dL3/2B4/dL5/2W6/dL7/2B
8/dL9/2B0
-----END PRIVATE KEY-----"#;

        let mut cert_file = NamedTempFile::new()?;
        cert_file.write_all(cert_pem.as_bytes())?;

        let mut key_file = NamedTempFile::new()?;
        key_file.write_all(key_pem.as_bytes())?;

        Ok((cert_file, key_file))
    }

    #[test]
    fn test_ssl_certificate_manager_creation() {
        let manager = SslCertificateManager::new();
        assert!(!manager.has_certificates());
        assert!(manager.list_certificates().is_empty());
    }

    #[test]
    fn test_ssl_redirect_config() {
        let config = SslRedirectConfig::new(true, 443);
        assert!(config.enabled);
        assert_eq!(config.https_port, 443);
        assert!(config.is_exempt("/health"));
        assert!(config.is_exempt("/ping"));
        assert!(!config.is_exempt("/api/users"));
    }
}
