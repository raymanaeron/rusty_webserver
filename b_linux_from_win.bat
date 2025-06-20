@echo off

REM Build script for Linux target from Windows
echo Building HTTP Server for Linux from Windows...

REM Parse command line arguments
set BUILD_TYPE=debug
set BUILD_FLAG=

if "%1"=="--release" (
    set BUILD_TYPE=release
    set BUILD_FLAG=--release
)

REM Check for better cross-compilation tools
where cargo-zigbuild >nul 2>&1
set HAS_ZIGBUILD=%errorlevel%

where cross >nul 2>&1
set HAS_CROSS=%errorlevel%

where wsl >nul 2>&1
set HAS_WSL=%errorlevel%

where docker >nul 2>&1
set HAS_DOCKER=%errorlevel%

echo Build type: %BUILD_TYPE%
echo.

echo =====================================================================
echo IMPORTANT: Cross-compiling from Windows to Linux is challenging
echo due to dependencies requiring Linux-specific build tools.
echo.
echo Here are your options:
echo.

if %HAS_WSL% equ 0 (
    echo Option 1 [RECOMMENDED/AVAILABLE]: Use WSL (Windows Subsystem for Linux):
    echo   wsl -e ./b_linux.sh %1
) else (
    echo Option 1 [RECOMMENDED/NOT INSTALLED]: Install WSL (Windows Subsystem for Linux):
    echo   wsl --install
    echo   Then: wsl -e ./b_linux.sh %1
)
echo.

if %HAS_DOCKER% equ 0 (
    echo Option 2 [RECOMMENDED/AVAILABLE]: Use Docker:
    echo   docker run --rm -v "%cd%:/app" -w /app rust:slim ./b_linux.sh %1
) else (
    echo Option 2 [RECOMMENDED/NOT INSTALLED]: Install Docker Desktop:
    echo   https://www.docker.com/products/docker-desktop/
    echo   Then: docker run --rm -v "%cd%:/app" -w /app rust:slim ./b_linux.sh %1
)
echo.

if %HAS_ZIGBUILD% equ 0 (
    echo Option 3 [AVAILABLE]: Use cargo-zigbuild (already installed):
    echo   cargo zigbuild --target=x86_64-unknown-linux-gnu %BUILD_FLAG%
) else (
    if %HAS_CROSS% equ 0 (
        echo Option 3 [AVAILABLE]: Use cross (already installed):
        echo   cross build --target=x86_64-unknown-linux-gnu %BUILD_FLAG%
    ) else (
        echo Option 3 [NOT INSTALLED]: Install specialized cross-compilation tools:
        echo   cargo install cross cargo-zigbuild
        echo   Then: cargo zigbuild --target=x86_64-unknown-linux-gnu %BUILD_FLAG%
        echo   Or: cross build --target=x86_64-unknown-linux-gnu %BUILD_FLAG%
    )
)
echo.
echo Option 4 [EASIEST]: Use GitHub Actions to build on Linux runners
echo   (Set up a CI workflow that builds on ubuntu-latest)
echo =====================================================================
echo.

REM Ask the user if they want to continue with direct cross-compilation
set /p CONTINUE=Do you want to attempt direct cross-compilation anyway? (y/N): 

if /i "%CONTINUE%"=="y" (
    REM Install Rust target for Linux
    echo Installing Linux target...
    rustup target add x86_64-unknown-linux-gnu
    
    REM Set up cross-compilation environment
    echo Setting up cross-compilation environment...
    
    REM Create .cargo directory if it doesn't exist
    if not exist .\.cargo mkdir .\.cargo
      REM Create config file with proper cross-compilation settings
    echo [target.x86_64-unknown-linux-gnu] > .\.cargo\config.toml
    echo linker = "rust-lld" >> .\.cargo\config.toml
    echo rustflags = ["-C", "link-arg=-fuse-ld=lld"] >> .\.cargo\config.toml
    
    REM First attempt: Try to build with specific feature flags to avoid problematic dependencies
    echo Attempt 1: Building with minimal features...
    cargo build %BUILD_FLAG% --target=x86_64-unknown-linux-gnu --no-default-features
    
    if %errorlevel% neq 0 (
        echo First attempt failed, trying alternative approach...
        
        REM Set environment variables to help with cross-compilation
        set CC_x86_64_unknown_linux_gnu=clang
        set CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=rust-lld
        
        echo Attempt 2: Building with cargo-zigbuild if available...
        where cargo-zigbuild >nul 2>&1
        if %errorlevel% equ 0 (
            echo Using cargo-zigbuild for better cross-compilation support...
            cargo zigbuild %BUILD_FLAG% --target=x86_64-unknown-linux-gnu
        ) else (
            echo cargo-zigbuild not found, trying with standard cargo...
            cargo build %BUILD_FLAG% --target=x86_64-unknown-linux-gnu
        )
    )
    
    if %errorlevel% equ 0 (
        echo Linux build successful
        echo Binary available at target\x86_64-unknown-linux-gnu\%BUILD_TYPE%\httpserver
        
        REM Copy app_config.toml to build directory
        echo Copying app_config.toml to build directory...
        if exist app_config.toml (
            copy app_config.toml target\x86_64-unknown-linux-gnu\%BUILD_TYPE%\app_config.toml >nul
            echo Configuration file copied to target\x86_64-unknown-linux-gnu\%BUILD_TYPE%\app_config.toml
        ) else (
            echo Warning: app_config.toml not found, application will use defaults
        )
        
        echo.
        echo Usage on Linux host:
        echo   ./httpserver --help
        echo   ./httpserver --directory ./public --port 8080
        echo   ./httpserver --config config.simple.toml
        echo.
        echo To upload to Linux host:
        echo   scp target\x86_64-unknown-linux-gnu\%BUILD_TYPE%\httpserver username@hostname:/path/on/linux/
        echo   scp target\x86_64-unknown-linux-gnu\%BUILD_TYPE%\app_config.toml username@hostname:/path/on/linux/ ^(if needed^)
    ) else (        echo.
        echo ===================================================================
        echo Build failed - this is expected due to crypto/SSL dependencies
        echo requiring a proper Linux toolchain.
        echo.
        echo To properly cross-compile this project, try these options:
        echo.
        echo 1. Install cargo-zigbuild: cargo install cargo-zigbuild
        echo    Then run this script again
        echo.
        echo 2. Use WSL: wsl -e ./b_linux.sh %1
        echo.
        echo 3. Use Docker: docker run --rm -v "%cd%:/app" -w /app rust:slim ./b_linux.sh %1
        echo.
        echo 4. Install cross: cargo install cross
        echo    Then use: cross build --target=x86_64-unknown-linux-gnu %BUILD_FLAG%
        echo ===================================================================
        exit /b 1
    )
) else (
    echo.
    echo Cross-compilation aborted.
    
    if %HAS_WSL% equ 0 (
        echo.
        echo Recommended action: Use WSL to build natively
        echo   wsl -e ./b_linux.sh %1
    ) else if %HAS_DOCKER% equ 0 (
        echo.
        echo Recommended action: Use Docker to build
        echo   docker run --rm -v "%cd%:/app" -w /app rust:slim ./b_linux.sh %1
    ) else if %HAS_ZIGBUILD% equ 0 (
        echo.
        echo Recommended action: Use cargo-zigbuild
        echo   cargo zigbuild --target=x86_64-unknown-linux-gnu %BUILD_FLAG%
    ) else if %HAS_CROSS% equ 0 (
        echo.
        echo Recommended action: Use cross
        echo   cross build --target=x86_64-unknown-linux-gnu %BUILD_FLAG%
    ) else (
        echo.
        echo Recommended action: Install cargo-zigbuild or cross
        echo   cargo install cargo-zigbuild
        echo   or
        echo   cargo install cross
    )
    exit /b 0
)
