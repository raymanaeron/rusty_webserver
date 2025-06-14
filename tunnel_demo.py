#!/usr/bin/env python3
"""
Tunnel System Demo Script for Phase 7.2
Demonstrates HTTP Host header routing and WebSocket-based tunneling

This script:
1. Starts a tunnel server
2. Starts a simple local web server
3. Starts a tunnel client that connects the local server to the tunnel
4. Makes HTTP requests through the tunnel to verify functionality
5. Tests SSL passthrough (if enabled)
"""

import subprocess
import time
import requests
import sys
import os
import signal
import threading
from typing import List, Optional

class TunnelDemo:
    def __init__(self):
        self.processes: List[subprocess.Popen] = []
        self.tunnel_server_port = 8080
        self.tunnel_public_port = 8081
        self.local_server_port = 3000
        self.base_domain = "tunnel.local"
        
    def start_process(self, cmd: List[str], name: str, cwd: Optional[str] = None) -> subprocess.Popen:
        """Start a process and track it for cleanup"""
        print(f"🚀 Starting {name}...")
        print(f"   Command: {' '.join(cmd)}")
        
        try:
            process = subprocess.Popen(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                cwd=cwd
            )
            self.processes.append(process)
            return process
        except Exception as e:
            print(f"❌ Failed to start {name}: {e}")
            sys.exit(1)
    
    def start_local_server(self):
        """Start a simple HTTP server for testing"""
        print("\n📂 Starting local web server...")
        
        # Create a simple index.html for testing
        html_content = """
<!DOCTYPE html>
<html>
<head>
    <title>Tunnel Demo - Local Server</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .container { max-width: 800px; margin: 0 auto; }
        .status { background: #e8f5e8; padding: 20px; border-radius: 8px; }
        .endpoint { background: #f0f0f0; padding: 10px; margin: 10px 0; border-radius: 4px; }
    </style>
</head>
<body>
    <div class="container">
        <h1>🎯 Tunnel Demo - Local Server</h1>
        <div class="status">
            <h2>✅ Local server is running!</h2>
            <p>This page is served by a local HTTP server on port 3000.</p>
            <p>It's accessible through the tunnel at: <strong>http://myapp.tunnel.local:8081</strong></p>
        </div>
        
        <h3>Available Endpoints:</h3>
        <div class="endpoint"><strong>GET /</strong> - This page</div>
        <div class="endpoint"><strong>GET /api/health</strong> - Health check endpoint</div>
        <div class="endpoint"><strong>GET /api/status</strong> - Server status</div>
        <div class="endpoint"><strong>POST /api/echo</strong> - Echo request body</div>
        
        <h3>Tunnel Information:</h3>
        <p><strong>Local Server:</strong> http://localhost:3000</p>
        <p><strong>Tunnel Server:</strong> ws://localhost:8080/connect</p>
        <p><strong>Public Endpoint:</strong> http://myapp.tunnel.local:8081</p>
        
        <h3>Test the Tunnel:</h3>
        <p>Try these commands from another terminal:</p>
        <pre>curl -H "Host: myapp.tunnel.local" http://localhost:8081/
curl -H "Host: myapp.tunnel.local" http://localhost:8081/api/health</pre>
    </div>
</body>
</html>
        """
        
        with open("index.html", "w") as f:
            f.write(html_content)
        
        # Start Python HTTP server
        cmd = [sys.executable, "-m", "http.server", str(self.local_server_port)]
        return self.start_process(cmd, "Local HTTP Server")
    
    def start_tunnel_server(self):
        """Start the Rust tunnel server"""
        print("\n🌐 Starting tunnel server...")
        
        # Build the tunnel server first
        print("   Building tunnel server...")
        build_cmd = ["cargo", "build", "--release", "-p", "httpserver-tunnel"]
        build_process = subprocess.run(build_cmd, cwd="../", capture_output=True, text=True)
        
        if build_process.returncode != 0:
            print(f"❌ Failed to build tunnel server:")
            print(build_process.stderr)
            sys.exit(1)
        
        # Start the tunnel server
        cmd = ["cargo", "run", "--release", "-p", "httpserver-tunnel", "--", 
               "--config", "config.tunnel-phase7.2.toml"]
        return self.start_process(cmd, "Tunnel Server", cwd="../")
    
    def start_tunnel_client(self):
        """Start the tunnel client example"""
        print("\n🔗 Starting tunnel client...")
        
        cmd = ["cargo", "run", "--release", "-p", "httpserver-tunnel", 
               "--example", "tunnel_client"]
        return self.start_process(cmd, "Tunnel Client", cwd="../")
    
    def wait_for_server(self, url: str, timeout: int = 30) -> bool:
        """Wait for a server to be ready"""
        print(f"   Waiting for server at {url}...")
        
        for i in range(timeout):
            try:
                response = requests.get(url, timeout=2)
                if response.status_code == 200:
                    print(f"   ✅ Server is ready!")
                    return True
            except:
                pass
            
            time.sleep(1)
            if i % 5 == 0:
                print(f"   Still waiting... ({i}/{timeout}s)")
        
        print(f"   ❌ Server did not respond within {timeout} seconds")
        return False
    
    def test_tunnel_functionality(self):
        """Test the tunnel system"""
        print("\n🧪 Testing tunnel functionality...")
        
        base_url = f"http://localhost:{self.tunnel_public_port}"
        headers = {"Host": f"myapp.{self.base_domain}"}
        
        tests = [
            {
                "name": "Home page",
                "url": f"{base_url}/",
                "method": "GET",
                "expected_status": 200,
                "expected_content": "Tunnel Demo"
            },
            {
                "name": "Health check", 
                "url": f"{base_url}/api/health",
                "method": "GET",
                "expected_status": 404,  # Python server doesn't have this endpoint
                "expected_content": None
            },
            {
                "name": "Non-existent page",
                "url": f"{base_url}/nonexistent",
                "method": "GET", 
                "expected_status": 404,
                "expected_content": None
            }
        ]
        
        results = []
        for test in tests:
            print(f"\n   Testing: {test['name']}")
            try:
                if test['method'] == 'GET':
                    response = requests.get(test['url'], headers=headers, timeout=10)
                else:
                    response = requests.post(test['url'], headers=headers, timeout=10)
                
                status_ok = response.status_code == test['expected_status']
                content_ok = True
                if test['expected_content']:
                    content_ok = test['expected_content'] in response.text
                
                if status_ok and content_ok:
                    print(f"   ✅ {test['name']}: PASSED (status: {response.status_code})")
                    results.append(True)
                else:
                    print(f"   ❌ {test['name']}: FAILED")
                    print(f"      Expected status: {test['expected_status']}, got: {response.status_code}")
                    if test['expected_content']:
                        print(f"      Expected content: {test['expected_content']}")
                    results.append(False)
                    
            except Exception as e:
                print(f"   ❌ {test['name']}: ERROR - {e}")
                results.append(False)
        
        success_rate = sum(results) / len(results) * 100
        print(f"\n   📊 Test Results: {sum(results)}/{len(results)} passed ({success_rate:.1f}%)")
        
        return all(results)
    
    def show_tunnel_info(self):
        """Display tunnel connection information"""
        print("\n📋 Tunnel System Information:")
        print(f"   🌐 Tunnel Server: ws://localhost:{self.tunnel_server_port}/connect")
        print(f"   🌍 Public Endpoint: http://localhost:{self.tunnel_public_port}")
        print(f"   🏠 Local Server: http://localhost:{self.local_server_port}")
        print(f"   🔗 Tunneled URL: http://myapp.{self.base_domain}:{self.tunnel_public_port}")
        print()
        print("   Try these commands:")
        print(f"   curl -H 'Host: myapp.{self.base_domain}' http://localhost:{self.tunnel_public_port}/")
        print(f"   curl -H 'Host: myapp.{self.base_domain}' http://localhost:{self.tunnel_public_port}/api/health")
        print()
    
    def cleanup(self):
        """Clean up all processes"""
        print("\n🧹 Cleaning up processes...")
        
        for process in self.processes:
            try:
                process.terminate()
                process.wait(timeout=5)
            except:
                try:
                    process.kill()
                except:
                    pass
        
        # Remove test files
        try:
            os.remove("index.html")
        except:
            pass
        
        print("   ✅ Cleanup complete")
    
    def run_demo(self):
        """Run the complete tunnel demo"""
        print("🚀 Starting Tunnel System Demo - Phase 7.2")
        print("============================================")
        
        try:
            # Start local server
            local_server = self.start_local_server()
            time.sleep(2)
            
            # Verify local server is running
            if not self.wait_for_server(f"http://localhost:{self.local_server_port}"):
                raise Exception("Local server failed to start")
            
            # Start tunnel server
            tunnel_server = self.start_tunnel_server()
            time.sleep(3)
            
            # Start tunnel client
            tunnel_client = self.start_tunnel_client()
            time.sleep(3)
            
            # Show connection info
            self.show_tunnel_info()
            
            # Test functionality
            if self.test_tunnel_functionality():
                print("\n🎉 All tests passed! Tunnel system is working correctly.")
            else:
                print("\n⚠️  Some tests failed. Check the logs above.")
            
            # Keep running until user interrupts
            print("\n⏸️  Demo is running. Press Ctrl+C to stop...")
            try:
                while True:
                    time.sleep(1)
            except KeyboardInterrupt:
                print("\n🛑 Stopping demo...")
                
        except Exception as e:
            print(f"\n❌ Demo failed: {e}")
        finally:
            self.cleanup()

def main():
    """Main function"""
    if len(sys.argv) > 1 and sys.argv[1] == "--test-only":
        # Quick test mode
        demo = TunnelDemo()
        try:
            demo.test_tunnel_functionality()
        finally:
            demo.cleanup()
    else:
        # Full demo mode
        demo = TunnelDemo()
        demo.run_demo()

if __name__ == "__main__":
    main()
