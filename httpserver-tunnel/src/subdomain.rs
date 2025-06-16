//! Subdomain Management System
//! Phase 7.2 - Advanced subdomain allocation with pronounceable words and persistence

use crate::{TunnelError, config::TunnelServerConfig};
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::fs;
use rand::{Rng, thread_rng};
use tracing::info;
use uuid::Uuid;

/// Subdomain allocation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubdomainRecord {
    pub subdomain: String,
    pub tunnel_id: String,
    pub allocated_at: chrono::DateTime<chrono::Utc>,
    pub is_custom: bool,
    pub client_ip: Option<String>,
}

/// Persistent subdomain storage
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubdomainStorage {
    pub active_subdomains: HashMap<String, SubdomainRecord>,
    pub reserved_subdomains: HashSet<String>, // Reserved words that cannot be allocated
    pub allocation_history: Vec<SubdomainRecord>, // Past allocations for analytics
}

/// Subdomain manager with persistence and word generation
#[derive(Debug)]
pub struct SubdomainManager {
    storage: Arc<RwLock<SubdomainStorage>>,
    storage_path: PathBuf,
    word_list: Vec<String>,
    config: TunnelServerConfig,
}

impl SubdomainManager {
    /// Create new subdomain manager
    pub fn new(config: TunnelServerConfig, storage_path: PathBuf) -> Self {
        let word_list = Self::generate_word_list();
        
        Self {
            storage: Arc::new(RwLock::new(SubdomainStorage::default())),
            storage_path,
            word_list,
            config,
        }
    }

    /// Initialize subdomain manager and load persistent data
    pub async fn initialize(&self) -> Result<(), TunnelError> {
        info!("Initializing subdomain manager with storage: {}", self.storage_path.display());
        
        // Load existing data if file exists
        if self.storage_path.exists() {
            self.load_storage().await?;
            info!("Loaded existing subdomain allocations from storage");
        } else {
            // Create storage directory if it doesn't exist
            if let Some(parent) = self.storage_path.parent() {
                fs::create_dir_all(parent).await
                    .map_err(|e| TunnelError::IoError(format!("Failed to create storage directory: {}", e)))?;
            }
            
            // Initialize with reserved words
            self.initialize_reserved_words().await;
            self.save_storage().await?;
            info!("Initialized new subdomain storage");
        }
        
        Ok(())
    }

    /// Generate a new subdomain based on configuration strategy
    pub async fn allocate_subdomain(
        &self,
        tunnel_id: &str,
        requested_subdomain: Option<String>,
        client_ip: Option<String>,
    ) -> Result<String, TunnelError> {
        match requested_subdomain {
            Some(custom) => {
                // Validate and allocate custom subdomain
                self.allocate_custom_subdomain(tunnel_id, &custom, client_ip).await
            }
            None => {
                // Generate random subdomain based on strategy
                self.allocate_random_subdomain(tunnel_id, client_ip).await
            }
        }
    }

    /// Allocate custom user-specified subdomain
    async fn allocate_custom_subdomain(
        &self,
        tunnel_id: &str,
        subdomain: &str,
        client_ip: Option<String>,
    ) -> Result<String, TunnelError> {
        // Validate subdomain format
        if !Self::is_valid_subdomain(subdomain) {
            return Err(TunnelError::ValidationError(
                "Invalid subdomain format. Use only lowercase letters, numbers, and hyphens.".to_string()
            ));
        }

        // Check if subdomain is available
        {
            let storage = self.storage.read().await;
            if storage.active_subdomains.contains_key(subdomain) {
                return Err(TunnelError::ConflictError(format!(
                    "Subdomain '{}' is already in use",
                    subdomain
                )));
            }
            
            if storage.reserved_subdomains.contains(subdomain) {
                return Err(TunnelError::ConflictError(format!(
                    "Subdomain '{}' is reserved and cannot be used",
                    subdomain
                )));
            }
        }

        // Allocate the subdomain
        let record = SubdomainRecord {
            subdomain: subdomain.to_string(),
            tunnel_id: tunnel_id.to_string(),
            allocated_at: chrono::Utc::now(),
            is_custom: true,
            client_ip,
        };

        {
            let mut storage = self.storage.write().await;
            storage.active_subdomains.insert(subdomain.to_string(), record.clone());
            storage.allocation_history.push(record);
        }

        self.save_storage().await?;
        
        info!("Allocated custom subdomain '{}' to tunnel {}", subdomain, tunnel_id);
        Ok(subdomain.to_string())
    }

    /// Allocate random subdomain using pronounceable words
    async fn allocate_random_subdomain(
        &self,
        tunnel_id: &str,
        client_ip: Option<String>,
    ) -> Result<String, TunnelError> {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 50;

        while attempts < MAX_ATTEMPTS {
            let subdomain = match self.config.subdomain_strategy {
                crate::config::SubdomainStrategy::Random => {
                    self.generate_pronounceable_subdomain()
                }
                crate::config::SubdomainStrategy::Uuid => {
                    self.generate_uuid_subdomain()
                }
                crate::config::SubdomainStrategy::UserSpecified => {
                    // Fallback to random for auto-allocation
                    self.generate_pronounceable_subdomain()
                }
            };

            // Check availability
            {
                let storage = self.storage.read().await;
                if !storage.active_subdomains.contains_key(&subdomain) 
                    && !storage.reserved_subdomains.contains(&subdomain) {
                    
                    // Allocate the subdomain
                    drop(storage);
                    let record = SubdomainRecord {
                        subdomain: subdomain.clone(),
                        tunnel_id: tunnel_id.to_string(),
                        allocated_at: chrono::Utc::now(),
                        is_custom: false,
                        client_ip,
                    };

                    {
                        let mut storage = self.storage.write().await;
                        storage.active_subdomains.insert(subdomain.clone(), record.clone());
                        storage.allocation_history.push(record);
                    }

                    self.save_storage().await?;
                    
                    info!("Allocated random subdomain '{}' to tunnel {}", subdomain, tunnel_id);
                    return Ok(subdomain);
                }
            }

            attempts += 1;
        }

        Err(TunnelError::InternalError(
            "Failed to generate unique subdomain after multiple attempts".to_string()
        ))
    }

    /// Generate pronounceable subdomain with word + numbers
    fn generate_pronounceable_subdomain(&self) -> String {
        let mut rng = thread_rng();
        
        // Select a random word from our word list
        let word = &self.word_list[rng.gen_range(0..self.word_list.len())];
        
        // Add 2-3 digit number for uniqueness
        let number = rng.gen_range(10..1000);
        
        format!("{}{}", word, number)
    }

    /// Generate UUID-based subdomain
    fn generate_uuid_subdomain(&self) -> String {
        Uuid::new_v4().to_string().replace('-', "")[..8].to_string()
    }

    /// Check if a subdomain is available
    pub async fn is_subdomain_available(&self, subdomain: &str) -> bool {
        let storage = self.storage.read().await;
        !storage.active_subdomains.contains_key(subdomain) 
            && !storage.reserved_subdomains.contains(subdomain)
    }

    /// Release a subdomain when tunnel disconnects
    pub async fn release_subdomain(&self, subdomain: &str) -> Result<(), TunnelError> {
        {
            let mut storage = self.storage.write().await;
            if let Some(record) = storage.active_subdomains.remove(subdomain) {
                info!("Released subdomain '{}' from tunnel {}", subdomain, record.tunnel_id);
            }
        }

        self.save_storage().await?;
        Ok(())
    }

    /// Get tunnel ID for a subdomain
    pub async fn get_tunnel_for_subdomain(&self, subdomain: &str) -> Option<String> {
        let storage = self.storage.read().await;
        storage.active_subdomains.get(subdomain).map(|record| record.tunnel_id.clone())
    }

    /// Get tunnel ID for a custom domain
    pub async fn get_tunnel_for_custom_domain(&self, domain: &str) -> Option<String> {
        // For now, we'll store custom domains in the same storage with a special prefix
        // In a production system, this might be a separate storage system
        let custom_key = format!("custom:{}", domain);
        let storage = self.storage.read().await;
        storage.active_subdomains.get(&custom_key).map(|record| record.tunnel_id.clone())
    }

    /// Allocate custom domain (for future use)
    pub async fn allocate_custom_domain(
        &self,
        tunnel_id: &str,
        domain: &str,
        client_ip: Option<String>,
    ) -> Result<String, TunnelError> {
        // Basic domain validation
        if domain.is_empty() || domain.len() > 253 {
            return Err(TunnelError::ValidationError("Invalid domain format".to_string()));
        }

        let custom_key = format!("custom:{}", domain);
        
        // Check if domain is available
        {
            let storage = self.storage.read().await;
            if storage.active_subdomains.contains_key(&custom_key) {
                return Err(TunnelError::ConflictError(format!(
                    "Custom domain '{}' is already in use",
                    domain
                )));
            }
        }

        // Allocate the custom domain
        let record = SubdomainRecord {
            subdomain: custom_key.clone(),
            tunnel_id: tunnel_id.to_string(),
            allocated_at: chrono::Utc::now(),
            is_custom: true,
            client_ip,
        };

        {
            let mut storage = self.storage.write().await;
            storage.active_subdomains.insert(custom_key, record.clone());
            storage.allocation_history.push(record);
        }

        self.save_storage().await?;
        
        info!("Allocated custom domain '{}' to tunnel {}", domain, tunnel_id);
        Ok(domain.to_string())
    }

    /// Get all active subdomains
    pub async fn get_active_subdomains(&self) -> HashMap<String, SubdomainRecord> {
        let storage = self.storage.read().await;
        storage.active_subdomains.clone()
    }

    /// Validate subdomain format
    fn is_valid_subdomain(subdomain: &str) -> bool {
        // Basic validation: 3-30 chars, lowercase alphanumeric + hyphens
        if subdomain.len() < 3 || subdomain.len() > 30 {
            return false;
        }

        if subdomain.starts_with('-') || subdomain.ends_with('-') {
            return false;
        }

        subdomain.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    }

    /// Load storage from file
    async fn load_storage(&self) -> Result<(), TunnelError> {
        let content = fs::read_to_string(&self.storage_path).await
            .map_err(|e| TunnelError::IoError(format!("Failed to read storage file: {}", e)))?;

        let storage_data: SubdomainStorage = serde_json::from_str(&content)
            .map_err(|e| TunnelError::SerializationError(format!("Failed to parse storage file: {}", e)))?;

        {
            let mut storage = self.storage.write().await;
            *storage = storage_data;
        }

        Ok(())
    }

    /// Save storage to file
    async fn save_storage(&self) -> Result<(), TunnelError> {
        let storage = self.storage.read().await;
        let content = serde_json::to_string_pretty(&*storage)
            .map_err(|e| TunnelError::SerializationError(format!("Failed to serialize storage: {}", e)))?;
        
        drop(storage);

        fs::write(&self.storage_path, content).await
            .map_err(|e| TunnelError::IoError(format!("Failed to write storage file: {}", e)))?;

        Ok(())
    }

    /// Initialize reserved words that cannot be allocated
    async fn initialize_reserved_words(&self) {
        let reserved = vec![
            // System subdomains
            "www", "api", "admin", "app", "mail", "ftp", "ssh",
            "vpn", "cdn", "static", "assets", "img", "images",
            "css", "js", "media", "files", "download", "upload",
            
            // Security-related
            "security", "auth", "login", "oauth", "sso", "saml",
            "ldap", "ad", "cert", "ssl", "tls", "key", "secret",
            
            // Infrastructure
            "proxy", "gateway", "load", "balance", "cache", "redis",
            "db", "database", "mysql", "postgres", "mongo", "elastic",
            "search", "log", "logs", "metrics", "monitor", "health",
            
            // Common services
            "dashboard", "console", "control", "manage", "config",
            "settings", "profile", "account", "user", "users",
            "webhook", "callback", "notify", "alert", "status",
            
            // Tunnel-specific
            "tunnel", "connect", "client", "server", "endpoint"
        ];

        let mut storage = self.storage.write().await;
        storage.reserved_subdomains.extend(reserved.iter().map(|s| s.to_string()));
    }

    /// Generate word list for pronounceable subdomains
    fn generate_word_list() -> Vec<String> {
        vec![
            // Adjectives (descriptive, positive)
            "mighty", "brave", "swift", "clever", "bright", "strong", "gentle", "noble",
            "quick", "smart", "bold", "calm", "cool", "fresh", "sharp", "smooth",
            "warm", "wise", "clear", "fast", "light", "pure", "safe", "solid",
            "super", "ultra", "mega", "prime", "elite", "royal", "grand", "magic",
            
            // Nouns (friendly, tech-related)
            "lion", "tiger", "eagle", "wolf", "bear", "fox", "hawk", "shark",
            "star", "moon", "sun", "storm", "wind", "fire", "rock", "wave",
            "code", "data", "link", "node", "core", "zone", "base", "port",
            "key", "lock", "gate", "path", "bridge", "tower", "space", "cloud",
            
            // Tech terms (modern, concise)
            "byte", "chip", "disk", "mesh", "grid", "sync", "flow", "beam",
            "pulse", "spark", "flash", "boost", "peak", "apex", "edge", "vertex",
            "pixel", "vector", "matrix", "tensor", "neural", "quantum", "digital", "cyber",
            
            // Colors (vibrant)
            "red", "blue", "green", "gold", "silver", "purple", "orange", "pink",
            "coral", "azure", "crimson", "emerald", "amber", "violet", "indigo", "cyan"
        ].iter().map(|s| s.to_string()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_manager() -> (SubdomainManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("subdomains.json");
        
        let config = TunnelServerConfig {
            enabled: true,
            subdomain_strategy: crate::config::SubdomainStrategy::Random,
            ..Default::default()
        };
        
        let manager = SubdomainManager::new(config, storage_path);
        manager.initialize().await.unwrap();
        
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_subdomain_manager_creation() {
        let (manager, _temp_dir) = create_test_manager().await;
        assert!(manager.word_list.len() > 50);
    }

    #[tokio::test]
    async fn test_custom_subdomain_allocation() {
        let (manager, _temp_dir) = create_test_manager().await;
        
        let subdomain = manager.allocate_subdomain(
            "test-tunnel-1", 
            Some("custom123".to_string()), 
            Some("127.0.0.1".to_string())
        ).await.unwrap();
        
        assert_eq!(subdomain, "custom123");
        assert!(!manager.is_subdomain_available("custom123").await);
    }

    #[tokio::test]
    async fn test_random_subdomain_allocation() {
        let (manager, _temp_dir) = create_test_manager().await;
        
        let subdomain1 = manager.allocate_subdomain(
            "test-tunnel-1", 
            None, 
            Some("127.0.0.1".to_string())
        ).await.unwrap();
        
        let subdomain2 = manager.allocate_subdomain(
            "test-tunnel-2", 
            None, 
            Some("127.0.0.1".to_string())
        ).await.unwrap();
        
        assert_ne!(subdomain1, subdomain2);
        assert!(subdomain1.len() >= 5); // word + numbers
        assert!(subdomain2.len() >= 5);
    }

    #[tokio::test]
    async fn test_subdomain_conflict() {
        let (manager, _temp_dir) = create_test_manager().await;
        
        // Allocate first subdomain
        manager.allocate_subdomain(
            "test-tunnel-1", 
            Some("conflict".to_string()), 
            None
        ).await.unwrap();
        
        // Try to allocate same subdomain
        let result = manager.allocate_subdomain(
            "test-tunnel-2", 
            Some("conflict".to_string()), 
            None
        ).await;
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TunnelError::ConflictError(_)));
    }

    #[tokio::test]
    async fn test_reserved_subdomain() {
        let (manager, _temp_dir) = create_test_manager().await;
        
        let result = manager.allocate_subdomain(
            "test-tunnel-1", 
            Some("admin".to_string()), 
            None
        ).await;
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TunnelError::ConflictError(_)));
    }

    #[tokio::test]
    async fn test_subdomain_release() {
        let (manager, _temp_dir) = create_test_manager().await;
        
        let subdomain = manager.allocate_subdomain(
            "test-tunnel-1", 
            Some("release-test".to_string()), 
            None
        ).await.unwrap();
        
        assert!(!manager.is_subdomain_available(&subdomain).await);
        
        manager.release_subdomain(&subdomain).await.unwrap();
        
        assert!(manager.is_subdomain_available(&subdomain).await);
    }

    #[tokio::test]
    async fn test_storage_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_subdomains.json");
        
        let config = TunnelServerConfig {
            enabled: true,
            subdomain_strategy: crate::config::SubdomainStrategy::Random,
            ..Default::default()
        };
        
        // Create first manager and allocate subdomain
        {
            let manager = SubdomainManager::new(config.clone(), storage_path.clone());
            manager.initialize().await.unwrap();
            
            manager.allocate_subdomain(
                "test-tunnel-1", 
                Some("persist-test".to_string()), 
                None
            ).await.unwrap();
        }
        
        // Create new manager and verify persistence
        {
            let manager = SubdomainManager::new(config, storage_path);
            manager.initialize().await.unwrap();
            
            assert!(!manager.is_subdomain_available("persist-test").await);
            assert_eq!(
                manager.get_tunnel_for_subdomain("persist-test").await,
                Some("test-tunnel-1".to_string())
            );
        }
    }
}
