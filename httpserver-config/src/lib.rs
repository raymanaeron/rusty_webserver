use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Command line arguments
#[derive(Parser)]
#[command(name = "httpserver")]
#[command(about = "A simple cross-platform HTTP server and gateway")]
pub struct Args {
    /// Directory to serve files from
    #[arg(short, long, default_value = ".")]
    pub directory: PathBuf,

    /// Port to listen on
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    /// Configuration file for proxy routes (future feature)
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}

/// Server configuration (for future phases)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Static file serving configuration
    pub static_config: StaticConfig,
    
    /// Proxy routes (future feature)
    #[serde(default)]
    pub proxy: Vec<ProxyRoute>,
}

/// Static file serving configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticConfig {
    /// Directory to serve files from
    pub directory: PathBuf,
    
    /// Fallback file for SPA support
    #[serde(default = "default_fallback")]
    pub fallback: String,
}

/// Proxy route configuration (future feature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyRoute {
    /// Path pattern to match
    pub path: String,
    
    /// Target URL or URLs
    pub target: String,
    
    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            static_config: StaticConfig {
                directory: PathBuf::from("."),
                fallback: "index.html".to_string(),
            },
            proxy: Vec::new(),
        }
    }
}

fn default_fallback() -> String {
    "index.html".to_string()
}

fn default_timeout() -> u64 {
    30
}

impl Config {
    /// Load configuration from TOML file
    pub fn load_from_file(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        // Read the configuration file
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file '{}': {}", path.display(), e))?;
        
        // Parse TOML content
        let config: Config = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse TOML in '{}': {}", path.display(), e))?;
        
        // Validate configuration
        config.validate()?;
        
        println!("Loaded configuration from: {}", path.display());
        Ok(config)
    }
    
    /// Create config from command line arguments
    pub fn from_args(args: Args) -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = if let Some(config_path) = &args.config {
            Self::load_from_file(config_path)?
        } else {
            Self::default()
        };
        
        // Override with CLI arguments
        config.static_config.directory = args.directory;
        
        Ok(config)
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Validate static directory exists
        if !self.static_config.directory.exists() {
            return Err(format!(
                "Static directory does not exist: {}", 
                self.static_config.directory.display()
            ).into());
        }
        
        // Validate proxy routes
        for (index, route) in self.proxy.iter().enumerate() {
            // Validate path pattern
            if route.path.is_empty() {
                return Err(format!("Proxy route {}: path cannot be empty", index).into());
            }
            
            // Validate target URL
            if route.target.is_empty() {
                return Err(format!("Proxy route {}: target cannot be empty", index).into());
            }
            
            // Basic URL validation
            if !route.target.starts_with("http://") && !route.target.starts_with("https://") {
                return Err(format!(
                    "Proxy route {}: target must be a valid HTTP/HTTPS URL: {}", 
                    index, route.target
                ).into());
            }
            
            // Validate timeout
            if route.timeout == 0 {
                return Err(format!("Proxy route {}: timeout must be greater than 0", index).into());
            }
        }
        
        println!("Configuration validation passed");
        Ok(())
    }
}
