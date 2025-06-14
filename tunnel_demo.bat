@echo off
REM Tunnel System Demo Script for Windows - Phase 7.2
REM Demonstrates HTTP Host header routing and WebSocket-based tunneling

echo 🚀 Starting Tunnel System Demo - Phase 7.2
echo ============================================

set TUNNEL_SERVER_PORT=8080
set TUNNEL_PUBLIC_PORT=8081
set LOCAL_SERVER_PORT=3000
set BASE_DOMAIN=tunnel.local

echo.
echo 📂 Starting local web server on port %LOCAL_SERVER_PORT%...

REM Create a simple index.html for testing
echo ^<!DOCTYPE html^> > index.html
echo ^<html^> >> index.html
echo ^<head^> >> index.html
echo     ^<title^>Tunnel Demo - Local Server^</title^> >> index.html
echo     ^<style^> >> index.html
echo         body { font-family: Arial, sans-serif; margin: 40px; } >> index.html
echo         .container { max-width: 800px; margin: 0 auto; } >> index.html
echo         .status { background: #e8f5e8; padding: 20px; border-radius: 8px; } >> index.html
echo     ^</style^> >> index.html
echo ^</head^> >> index.html
echo ^<body^> >> index.html
echo     ^<div class="container"^> >> index.html
echo         ^<h1^>🎯 Tunnel Demo - Local Server^</h1^> >> index.html
echo         ^<div class="status"^> >> index.html
echo             ^<h2^>✅ Local server is running!^</h2^> >> index.html
echo             ^<p^>This page is served by a local HTTP server on port 3000.^</p^> >> index.html
echo             ^<p^>It's accessible through the tunnel at: ^<strong^>http://myapp.tunnel.local:8081^</strong^>^</p^> >> index.html
echo         ^</div^> >> index.html
echo         ^<h3^>Test Commands:^</h3^> >> index.html
echo         ^<pre^>curl -H "Host: myapp.tunnel.local" http://localhost:8081/^</pre^> >> index.html
echo     ^</div^> >> index.html
echo ^</body^> >> index.html
echo ^</html^> >> index.html

REM Start Python HTTP server in background
start "Local HTTP Server" cmd /c "python -m http.server %LOCAL_SERVER_PORT%"

echo ⏳ Waiting for local server to start...
timeout /t 3 >nul

echo.
echo 🌐 Building and starting tunnel server...
cd httpserver-tunnel
cargo build --release
if errorlevel 1 (
    echo ❌ Failed to build tunnel server
    goto cleanup
)

REM Start tunnel server in background
start "Tunnel Server" cmd /c "cargo run --release -- --config ../config.tunnel-phase7.2.toml"

echo ⏳ Waiting for tunnel server to start...
timeout /t 5 >nul

echo.
echo 🔗 Starting tunnel client...
start "Tunnel Client" cmd /c "cargo run --example tunnel_client"

echo ⏳ Waiting for tunnel client to connect...
timeout /t 3 >nul

cd ..

echo.
echo 📋 Tunnel System Information:
echo    🌐 Tunnel Server: ws://localhost:%TUNNEL_SERVER_PORT%/connect
echo    🌍 Public Endpoint: http://localhost:%TUNNEL_PUBLIC_PORT%
echo    🏠 Local Server: http://localhost:%LOCAL_SERVER_PORT%
echo    🔗 Tunneled URL: http://myapp.%BASE_DOMAIN%:%TUNNEL_PUBLIC_PORT%
echo.
echo    Try these commands in another terminal:
echo    curl -H "Host: myapp.%BASE_DOMAIN%" http://localhost:%TUNNEL_PUBLIC_PORT%/
echo    curl -H "Host: myapp.%BASE_DOMAIN%" http://localhost:%TUNNEL_PUBLIC_PORT%/api/health
echo.

echo 🧪 Testing tunnel functionality...
echo.

REM Test home page
echo Testing home page...
curl -H "Host: myapp.%BASE_DOMAIN%" -s -w "Status: %%{http_code}\n" http://localhost:%TUNNEL_PUBLIC_PORT%/ > nul
if errorlevel 1 (
    echo ❌ Home page test failed
) else (
    echo ✅ Home page test passed
)

echo.
echo 🎉 Demo is running! 
echo.
echo Press any key to stop all services...
pause >nul

:cleanup
echo.
echo 🧹 Cleaning up...

REM Kill all related processes
taskkill /f /im python.exe 2>nul
taskkill /f /im cargo.exe 2>nul
taskkill /f /im httpserver-tunnel.exe 2>nul

REM Remove test file
del index.html 2>nul

echo ✅ Cleanup complete
echo.
echo 👋 Demo finished!
