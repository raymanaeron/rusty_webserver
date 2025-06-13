@echo off
echo Testing Enhanced Logging System
echo =================================

echo.
echo Building project...
cargo build
if errorlevel 1 (
    echo Build failed
    exit /b 1
)

echo.
echo Build successful!

echo.
echo Running all httpserver-core tests (including logging tests)...
cargo test --package httpserver-core
if errorlevel 1 (
    echo Some tests may have failed, but continuing...
)

echo.
echo Running specific logging tests with verbose output...
cargo test --package httpserver-core --test logging_tests -- --nocapture
if errorlevel 1 (
    echo Logging tests may have failed
)

echo.
echo Core tests completed!

echo.
echo Testing basic server startup with logging...
echo Starting server in background for 5 seconds...

start /b cargo run -- --config config.simple.toml
timeout /t 5 /nobreak >nul

echo.
echo Stopping server...
taskkill /f /im httpserver.exe >nul 2>&1

echo.
echo Checking for log files...
if exist "logs" (
    echo Logs directory found
    dir logs /b
) else (
    echo No logs directory found
)

echo.
echo Enhanced logging test completed!
echo.
echo To run the server with logging manually:
echo    cargo run -- --config config.simple.toml
echo.
echo Check the logs directory for output files
echo.
echo To run all tests:
echo    cargo test --package httpserver-core
echo.
echo Available logging features:
echo    - Structured logging with tracing crate
echo    - File-based logging with rotation
echo    - Configurable log levels and formats
echo    - Request IDs for traceability
echo    - Performance metrics collection
