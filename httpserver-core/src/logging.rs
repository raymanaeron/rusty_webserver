use std::path::Path;
use tracing_subscriber::{
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
    Layer,
};
use tracing_appender::{
    non_blocking,
    rolling::{RollingFileAppender, Rotation},
};
use httpserver_config::LoggingConfig;

/// Initialize the logging system based on configuration
pub fn initialize_logging(config: &LoggingConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Create logs directory if it doesn't exist
    if config.file_logging && (config.output_mode == "both" || config.output_mode == "file") {
        std::fs::create_dir_all(&config.logs_directory)?;
    }

    // Create the filter based on log level
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.level));

    // Build the subscriber
    let subscriber = tracing_subscriber::registry()
        .with(filter);

    // Determine what outputs to enable based on output_mode
    let enable_file = config.file_logging && (config.output_mode == "both" || config.output_mode == "file");
    let enable_console = config.output_mode == "both" || config.output_mode == "console";

    if enable_file && enable_console {
        // Both file and console output
        let file_appender = RollingFileAppender::builder()
            .rotation(Rotation::NEVER) // We'll handle rotation manually by size
            .filename_prefix("httpserver")
            .filename_suffix("log")
            .build(&config.logs_directory)?;

        let (non_blocking_appender, guard) = non_blocking(file_appender);
        
        // Keep the guard alive for the entire program duration
        // This is necessary to prevent the file logging from stopping
        std::mem::forget(guard);
        
        // Add file logging layer (NO ANSI colors for file output)
        let file_layer = if config.format == "json" {
            fmt::layer()
                .json()
                .with_ansi(false)  // Disable ANSI colors for file output
                .with_writer(non_blocking_appender)
                .boxed()
        } else {
            fmt::layer()
                .with_ansi(false)  // Disable ANSI colors for file output
                .with_writer(non_blocking_appender)
                .boxed()
        };

        // Add console logging layer (WITH ANSI colors for console output)
        let console_layer = if config.format == "json" {
            fmt::layer()
                .json()
                .with_ansi(true)   // Enable ANSI colors for console output
                .with_writer(std::io::stdout)
                .boxed()
        } else {
            fmt::layer()
                .with_ansi(true)   // Enable ANSI colors for console output
                .with_writer(std::io::stdout)
                .boxed()
        };

        subscriber
            .with(file_layer)
            .with(console_layer)
            .init();        tracing::info!(
            logs_directory = %config.logs_directory.display(),
            level = %config.level,
            format = %config.format,
            output_mode = %config.output_mode,
            structured_logging = config.structured_logging,
            enable_request_ids = config.enable_request_ids,
            enable_performance_metrics = config.enable_performance_metrics,
            "Logging initialized with both file and console output"
        );
    } else if enable_file {
        // File-only output
        let file_appender = RollingFileAppender::builder()
            .rotation(Rotation::NEVER) // We'll handle rotation manually by size
            .filename_prefix("httpserver")
            .filename_suffix("log")
            .build(&config.logs_directory)?;

        let (non_blocking_appender, guard) = non_blocking(file_appender);
        
        // Keep the guard alive for the entire program duration
        // This is necessary to prevent the file logging from stopping
        std::mem::forget(guard);
        
        let file_layer = if config.format == "json" {
            fmt::layer()
                .json()
                .with_ansi(false)  // Disable ANSI colors for file output
                .with_writer(non_blocking_appender)
                .boxed()
        } else {
            fmt::layer()
                .with_ansi(false)  // Disable ANSI colors for file output
                .with_writer(non_blocking_appender)
                .boxed()
        };

        subscriber
            .with(file_layer)
            .init();

        tracing::info!(
            logs_directory = %config.logs_directory.display(),
            level = %config.level,
            format = %config.format,
            output_mode = %config.output_mode,
            "Logging initialized with file output only"
        );
    } else {        // Console-only logging (WITH ANSI colors)
        let console_layer = if config.format == "json" {
            fmt::layer()
                .json()
                .with_ansi(true)   // Enable ANSI colors for console output
                .with_writer(std::io::stdout)
                .boxed()
        } else {
            fmt::layer()
                .with_ansi(true)   // Enable ANSI colors for console output
                .with_writer(std::io::stdout)
                .boxed()
        };

        subscriber
            .with(console_layer)
            .init();

        tracing::info!(
            level = %config.level,
            format = %config.format,
            output_mode = %config.output_mode,
            "Logging initialized (console only)"
        );
    }    Ok(())
}

/// Create a request span with unique ID for tracing
pub fn create_request_span(method: &str, path: &str, client_ip: &str) -> tracing::Span {
    let request_id = uuid::Uuid::new_v4().to_string();
    
    tracing::info_span!(
        "request",
        request_id = %request_id,
        method = %method,
        path = %path,
        client_ip = %client_ip
    )
}

/// Check and rotate log files if they exceed the size limit
pub fn check_log_rotation(config: &LoggingConfig) -> Result<(), Box<dyn std::error::Error>> {
    if !config.file_logging {
        return Ok(());
    }

    let log_file_path = config.logs_directory.join("httpserver.log");
    
    if let Ok(metadata) = std::fs::metadata(&log_file_path) {
        let size_mb = metadata.len() / (1024 * 1024);
        
        if size_mb >= config.file_size_mb {
            rotate_log_file(&log_file_path)?;
            tracing::info!(
                file_size_mb = size_mb,
                limit_mb = config.file_size_mb,
                "Log file rotated due to size limit"
            );
        }
    }

    Ok(())
}

/// Rotate log file by renaming to .1, .2, etc.
fn rotate_log_file(log_file_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Find the highest numbered backup
    let mut highest_backup = 0;
    let parent_dir = log_file_path.parent().unwrap();
    
    for entry in std::fs::read_dir(parent_dir)? {
        let entry = entry?;
        let filename = entry.file_name();
        let filename_str = filename.to_string_lossy();
        
        if filename_str.starts_with("httpserver.log.") {
            if let Some(num_str) = filename_str.strip_prefix("httpserver.log.") {
                if let Ok(num) = num_str.parse::<u32>() {
                    highest_backup = highest_backup.max(num);
                }
            }
        }
    }
    
    // Rotate existing backups
    for i in (1..=highest_backup).rev() {
        let old_path = parent_dir.join(format!("httpserver.log.{}", i));
        let new_path = parent_dir.join(format!("httpserver.log.{}", i + 1));
        
        if old_path.exists() {
            std::fs::rename(old_path, new_path)?;
        }
    }
    
    // Move current log to .1
    let backup_path = parent_dir.join("httpserver.log.1");
    std::fs::rename(log_file_path, backup_path)?;
    
    Ok(())
}

/// Clean up old log files based on retention policy
pub fn cleanup_old_logs(config: &LoggingConfig) -> Result<(), Box<dyn std::error::Error>> {
    if !config.file_logging {
        return Ok(());
    }

    let cutoff_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() - (config.retention_days as u64 * 24 * 60 * 60);

    let parent_dir = &config.logs_directory;
    
    for entry in std::fs::read_dir(parent_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if let Some(filename) = path.file_name() {
            let filename_str = filename.to_string_lossy();
            
            // Only clean up log files
            if filename_str.starts_with("httpserver.log.") {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified_time) = metadata.modified() {
                        let modified_secs = modified_time
                            .duration_since(std::time::UNIX_EPOCH)?
                            .as_secs();
                        
                        if modified_secs < cutoff_time {
                            std::fs::remove_file(&path)?;
                            tracing::info!(
                                file = %path.display(),
                                "Removed old log file"
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
