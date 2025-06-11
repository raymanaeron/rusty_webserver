@echo off
REM Build script for HTTP Server on Windows

echo Building HTTP Server...

REM Build for Windows
echo 📦 Building for Windows...
cargo build --release
if %errorlevel% neq 0 (
    echo Build failed
    pause
    exit /b 1
)

echo Build successful!

REM Create dist directory
if not exist "dist" mkdir dist

REM Copy binary
copy "target\release\httpserver.exe" "dist\httpserver-windows.exe"

echo Binary saved to dist\httpserver-windows.exe
echo.
echo Usage:
echo   dist\httpserver-windows.exe --help
echo   dist\httpserver-windows.exe --directory .\public --port 8080
echo.
pause
