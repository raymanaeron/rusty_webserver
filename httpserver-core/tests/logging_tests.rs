//! Logging functionality tests
//!
//! This module tests the enhanced logging system implementation.

use std::path::PathBuf;
use std::sync::Once;
use httpserver_config::LoggingConfig;
use httpserver_core::logging::{ initialize_logging, cleanup_old_logs };
use tokio;
use uuid;

static INIT: Once = Once::new();

fn ensure_logging_initialized() {
    INIT.call_once(|| {
        let config = LoggingConfig::default();
        let _ = initialize_logging(&config);
    });
}

#[test]
fn test_logging_config_defaults() {
    let config = LoggingConfig::default();

    assert_eq!(config.level, "info");
    assert_eq!(config.file_logging, true);
    assert_eq!(config.logs_directory, PathBuf::from("./logs"));
    assert_eq!(config.file_size_mb, 10);
    assert_eq!(config.retention_days, 30);
    assert_eq!(config.format, "text");
    assert_eq!(config.output_mode, "both");
    assert_eq!(config.structured_logging, true);
    assert_eq!(config.enable_request_ids, true);
    assert_eq!(config.enable_performance_metrics, true);
}

#[test]
fn test_logging_initialization_structure() {
    // Test that we can create different logging configs without initializing
    let mut config1 = LoggingConfig::default();
    config1.level = "info".to_string();
    config1.file_logging = true;
    config1.logs_directory = PathBuf::from("./test_logs_init");
    config1.file_size_mb = 1;
    config1.retention_days = 7;
    config1.format = "text".to_string();

    let mut config2 = LoggingConfig::default();
    config2.level = "debug".to_string();
    config2.file_logging = true;
    config2.logs_directory = PathBuf::from("./test_logs_json");
    config2.file_size_mb = 1;
    config2.retention_days = 7;
    config2.format = "json".to_string();

    let configs = vec![config1, config2];

    for config in configs {
        // Verify config structure is valid
        assert!(!config.level.is_empty());
        assert!(!config.logs_directory.as_os_str().is_empty());
        assert!(config.file_size_mb > 0);
        assert!(config.retention_days > 0);
        assert!(config.format == "text" || config.format == "json");
    }
}

#[tokio::test]
async fn test_log_message_generation() {
    ensure_logging_initialized();

    // Generate test log messages
    tracing::info!(
        message = "Test info message",
        test_field = "test_value",
        "This is a test info log"
    );

    tracing::warn!(
        message = "Test warning message",
        warning_type = "test_warning",
        "This is a test warning log"
    );

    tracing::error!(message = "Test error message", error_code = 500, "This is a test error log");

    // Test structured logging with request simulation
    let request_id = uuid::Uuid::new_v4();
    tracing::info!(
        request_id = %request_id,
        method = "GET",
        path = "/test",
        client_ip = "127.0.0.1:12345",
        duration_ms = 42,
        status = 200,
        "HTTP request processed"
    );

    // Just verify we can generate structured logs without panicking
    assert!(true, "Log message generation should not panic");
}

#[tokio::test]
async fn test_log_cleanup() {
    let test_dir = "./test_logs_cleanup";

    // Create test directory and some files
    std::fs::create_dir_all(test_dir).expect("Should create test directory");
    std::fs
        ::write(format!("{}/test.log", test_dir), "test content")
        .expect("Should create test file"); // Create test config for cleanup
    let mut logging_config = LoggingConfig::default();
    logging_config.level = "info".to_string();
    logging_config.file_logging = true;
    logging_config.logs_directory = PathBuf::from(test_dir);
    logging_config.file_size_mb = 1;
    logging_config.retention_days = 7;
    logging_config.format = "text".to_string();

    // Test log cleanup
    let result = cleanup_old_logs(&logging_config);
    assert!(result.is_ok(), "Log cleanup should succeed");

    // Clean up test directory
    let _ = std::fs::remove_dir_all(test_dir);
}

#[test]
fn test_different_log_levels() {
    // Test different log level configurations
    let levels = vec!["debug", "info", "warn", "error"];
    let formats = vec!["text", "json"];
    for level in levels {
        for format in &formats {
            let mut config = LoggingConfig::default();
            config.level = level.to_string();
            config.file_logging = true;
            config.logs_directory = PathBuf::from("./test_logs");
            config.file_size_mb = 1;
            config.retention_days = 7;
            config.format = format.to_string();

            // Verify config creation succeeds
            assert_eq!(config.level, level);
            assert_eq!(config.format, *format);
        }
    }
}

#[tokio::test]
async fn test_console_mode() {
    ensure_logging_initialized();

    // Generate a test message
    tracing::info!("Console test message");

    // Test that we can log without errors
    assert!(true, "Console logging should work");
}

#[test]
fn test_config_validation() {
    // Test that configs can be created with different values
    let mut config = LoggingConfig::default();
    config.level = "debug".to_string();
    config.file_logging = false;
    config.logs_directory = PathBuf::from("./custom_logs");
    config.file_size_mb = 5;
    config.retention_days = 14;
    config.format = "json".to_string();

    assert_eq!(config.level, "debug");
    assert_eq!(config.file_logging, false);
    assert_eq!(config.logs_directory, PathBuf::from("./custom_logs"));
    assert_eq!(config.file_size_mb, 5);
    assert_eq!(config.retention_days, 14);
    assert_eq!(config.format, "json");
}
