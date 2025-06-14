@echo off

REM Build script for Windows
echo Building HTTP Server for Windows...

REM Parse command line arguments
set BUILD_TYPE=debug
set BUILD_FLAG=

if "%1"=="--release" (
    set BUILD_TYPE=release
    set BUILD_FLAG=--release
)

echo Build type: %BUILD_TYPE%
echo.

REM Build for Windows
echo Building for Windows (%BUILD_TYPE%)...
cargo build %BUILD_FLAG%

if %errorlevel% equ 0 (
    echo Windows build successful
    echo Binary available at target\%BUILD_TYPE%\httpserver.exe
    
    REM Copy app_config.toml to build directory
    echo Copying app_config.toml to build directory...
    if exist app_config.toml (
        copy app_config.toml target\%BUILD_TYPE%\app_config.toml >nul
        echo Configuration file copied to target\%BUILD_TYPE%\app_config.toml
    ) else (
        echo Warning: app_config.toml not found, application will use defaults
    )
    
    echo.
    echo Usage:
    echo   .\target\%BUILD_TYPE%\httpserver.exe --help
    echo   .\target\%BUILD_TYPE%\httpserver.exe --directory .\public --port 8080
    echo   .\target\%BUILD_TYPE%\httpserver.exe --config config.simple.toml
) else (
    echo Windows build failed
    exit /b 1
)

echo Build completed!
echo.
