#!/usr/bin/env python3
"""
Phase 7.3 Tunnel Authentication Demo
Simple demonstration of API key and JWT authentication with the tunnel server
"""

import json
import jwt
import time
import websockets
import asyncio
from datetime import datetime, timedelta

# Configuration
TUNNEL_SERVER = "ws://localhost:8091/connect"
API_KEYS = [
    "sk-user123-abcdef123456",  # Will get subdomain: user123.httpserver.io
    "sk-alice-7890wxyz",        # Will get subdomain: alice.httpserver.io
    "demo-api-key-qwerty"       # Will get subdomain: demo-api.httpserver.io
]
JWT_SECRET = "your-super-secret-jwt-signing-key-change-this-in-production"

def create_jwt_token(username, expires_in_hours=24):
    """Create a JWT token for the given username"""
    payload = {
        "sub": username,           # Subject (user ID)
        "username": username,      # Username claim
        "exp": int(time.time()) + (expires_in_hours * 3600),  # Expiration
        "iat": int(time.time()),   # Issued at
        "jti": f"tunnel-{username}-{int(time.time())}"  # JWT ID
    }
    return jwt.encode(payload, JWT_SECRET, algorithm="HS256")

def create_auth_message(token, requested_subdomain=None):
    """Create tunnel authentication message"""
    return {
        "type": "Auth",
        "token": token,
        "subdomain": requested_subdomain,
        "protocol_version": "1.0"
    }

async def test_api_key_auth(api_key):
    """Test API key authentication"""
    print(f"\n🔑 Testing API Key Authentication: {api_key}")
    
    try:
        # Connect to tunnel server
        async with websockets.connect(TUNNEL_SERVER) as websocket:
            # Send authentication with API key
            auth_msg = create_auth_message(api_key)
            await websocket.send(json.dumps(auth_msg))
            
            # Wait for authentication response
            response = await websocket.recv()
            auth_response = json.loads(response)
            
            if auth_response.get("type") == "AuthResponse":
                if auth_response.get("success"):
                    subdomain = auth_response.get("assigned_subdomain")
                    print(f"✅ Authentication successful!")
                    print(f"   Assigned subdomain: {subdomain}")
                    print(f"   Public URL: https://{subdomain}.httpserver.io")
                else:
                    error = auth_response.get("error", "Unknown error")
                    print(f"❌ Authentication failed: {error}")
            else:
                print(f"❌ Unexpected response: {auth_response}")
                
    except Exception as e:
        print(f"❌ Connection failed: {e}")

async def test_jwt_auth(username):
    """Test JWT token authentication"""
    print(f"\n🎫 Testing JWT Token Authentication: {username}")
    
    try:
        # Create JWT token
        jwt_token = create_jwt_token(username)
        print(f"   Generated JWT: {jwt_token[:50]}...")
        
        # Connect to tunnel server
        async with websockets.connect(TUNNEL_SERVER) as websocket:
            # Send authentication with JWT token
            auth_msg = create_auth_message(jwt_token)
            await websocket.send(json.dumps(auth_msg))
            
            # Wait for authentication response
            response = await websocket.recv()
            auth_response = json.loads(response)
            
            if auth_response.get("type") == "AuthResponse":
                if auth_response.get("success"):
                    subdomain = auth_response.get("assigned_subdomain")
                    print(f"✅ Authentication successful!")
                    print(f"   Assigned subdomain: {subdomain}")
                    print(f"   Public URL: https://{subdomain}.httpserver.io")
                else:
                    error = auth_response.get("error", "Unknown error")
                    print(f"❌ Authentication failed: {error}")
            else:
                print(f"❌ Unexpected response: {auth_response}")
                
    except Exception as e:
        print(f"❌ Connection failed: {e}")

async def test_custom_subdomain_request():
    """Test requesting a custom subdomain"""
    print(f"\n🎯 Testing Custom Subdomain Request")
    
    try:
        api_key = API_KEYS[0]  # Use first API key
        requested_subdomain = "myapp"
        
        # Connect to tunnel server
        async with websockets.connect(TUNNEL_SERVER) as websocket:
            # Send authentication with custom subdomain request
            auth_msg = create_auth_message(api_key, requested_subdomain)
            await websocket.send(json.dumps(auth_msg))
            
            # Wait for authentication response
            response = await websocket.recv()
            auth_response = json.loads(response)
            
            if auth_response.get("type") == "AuthResponse":
                if auth_response.get("success"):
                    subdomain = auth_response.get("assigned_subdomain")
                    print(f"✅ Authentication successful!")
                    print(f"   Requested subdomain: {requested_subdomain}")
                    print(f"   Assigned subdomain: {subdomain}")
                    print(f"   Public URL: https://{subdomain}.httpserver.io")
                else:
                    error = auth_response.get("error", "Unknown error")
                    print(f"❌ Authentication failed: {error}")
            else:
                print(f"❌ Unexpected response: {auth_response}")
                
    except Exception as e:
        print(f"❌ Connection failed: {e}")

async def main():
    print("🚀 Phase 7.3 Tunnel Authentication Demo")
    print("=" * 50)
    
    print("\n📋 Configuration:")
    print(f"   Tunnel Server: {TUNNEL_SERVER}")
    print(f"   API Keys: {len(API_KEYS)} configured")
    print(f"   JWT Support: Enabled")
    
    # Test API key authentication
    for api_key in API_KEYS:
        await test_api_key_auth(api_key)
        await asyncio.sleep(1)  # Brief delay
    
    # Test JWT authentication
    jwt_users = ["alice", "bob", "charlie"]
    for username in jwt_users:
        await test_jwt_auth(username)
        await asyncio.sleep(1)  # Brief delay
    
    # Test custom subdomain request
    await test_custom_subdomain_request()
    
    print("\n🎉 Demo completed!")
    print("\nTo start the tunnel server with authentication:")
    print("cargo run --bin httpserver -- --config config.tunnel-auth-demo.toml")

if __name__ == "__main__":
    try:
        import jwt
        import websockets
    except ImportError as e:
        print(f"❌ Missing dependency: {e}")
        print("Install with: pip install PyJWT websockets")
        exit(1)
    
    asyncio.run(main())
