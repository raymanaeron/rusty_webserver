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
    echo.
    echo Usage:
    echo   .\target\%BUILD_TYPE%\httpserver.exe --help
    echo   .\target\%BUILD_TYPE%\httpserver.exe --directory .\public --port 8080
) else (
    echo Windows build failed
    exit /b 1
)

echo Build completed!
pause
