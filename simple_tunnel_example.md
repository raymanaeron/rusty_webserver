# Simple Tunnel Example: Exposing Local HttpServer Through Public Tunnel

This guide demonstrates how to set up a tunnel connection between a public-facing HttpServer (running on a Digital Ocean droplet) and a local HttpServer instance (running behind a firewall at home). The tunnel allows public traffic to be forwarded to your local server seamlessly.

## Overview

The setup consists of two HttpServer instances:

1. **Tunnel Server** (Digital Ocean Droplet) - Acts as the public gateway
2. **Tunnel Client** (Home Network) - Your local HttpServer that gets exposed

When users visit the public domain, traffic is tunneled through a WebSocket connection to your local server.

## Architecture Diagram

```
[Internet] → [Digital Ocean HttpServer (Tunnel Server)] 
              ↓ (WebSocket Tunnel)
              [Home HttpServer (Tunnel Client)]
```

## Prerequisites

1. Digital Ocean droplet with HttpServer installed
2. Local machine with HttpServer installed
3. Domain name pointing to your Digital Ocean droplet
4. Both instances have network connectivity

## Part 1: Setting Up the Tunnel Server (Digital Ocean Droplet)

### 1.1 Create Tunnel Server Configuration

Create `config.tunnel-server-production.toml` on your Digital Ocean droplet:

```toml
# Production Tunnel Server Configuration
# This runs on the Digital Ocean droplet

[logging]
level = "info"
file_logging = true
logs_directory = "./logs"
format = "text"
output_mode = "both"

[application]
name = "httpserver-tunnel-server"
environment = "production"

[server]
default_port = 80
request_timeout = 30
max_request_size_mb = 10
enable_health_endpoints = true

# DISABLE SSL for this example
[server.ssl]
enabled = false

[static_config]
directory = "./website"
fallback = "index.html"

# TUNNEL SERVER CONFIGURATION
[tunnel]
enabled = true

# This server acts as tunnel server
[tunnel.server]
# Enable tunnel server functionality
enabled = true

# Port for tunnel WebSocket connections (where clients connect)
tunnel_port = 8081

# Base domain for tunnel subdomains (replace with your domain)
base_domain = "yourdomain.com"

# Public HTTP port (where public traffic is served)
public_port = 80

# Public HTTPS port (disabled for this example)
public_https_port = 443

# Maximum number of concurrent tunnels
max_tunnels = 100

# How subdomains are allocated
subdomain_strategy = "Random"

# Length of random subdomains
subdomain_length = 8

# Reserved subdomains that cannot be allocated
reserved_subdomains = ["www", "api", "admin", "mail", "ftp", "app", "secure"]

# Authentication for tunnel connections
[tunnel.server.auth]
required = true
api_keys = ["your-secret-api-key-here", "backup-key-123"]

# Rate limiting to prevent abuse
[tunnel.server.rate_limit]
enabled = true
requests_per_minute = 1000
max_connections_per_tunnel = 50
max_bandwidth_per_tunnel = 10485760  # 10 MB/s

# DISABLE SSL for tunnel server
[tunnel.server.ssl]
enabled = false

# Monitoring and health checks
[tunnel.server.monitoring]
enabled = true
health_endpoint = "/health"
collect_metrics = true
metrics_endpoint = "/metrics"
log_tunnel_events = true

# Performance settings
[tunnel.server.performance]
connection_pool_size = 1000
worker_threads = 0  # Auto-detect
websocket_buffer_size = 65536
cleanup_interval = 300

# Security settings
[tunnel.server.security]
cors_enabled = true
cors_origins = []  # Allow all origins
validate_host_headers = true
max_request_size = 104857600  # 100 MB
connection_timeout = 60
idle_timeout = 300

# Network configuration
[tunnel.server.network]
bind_address = "0.0.0.0"
public_bind_address = "0.0.0.0"
tcp_keepalive = true
```

### 1.2 Start the Tunnel Server

On your Digital Ocean droplet:

```bash
# Start the tunnel server
./httpserver --config config.tunnel-server-production.toml --port 80
```

The server will:
- Listen on port 80 for public HTTP traffic
- Listen on port 8081 for tunnel WebSocket connections
- Accept tunnel client connections with valid API keys

### 1.3 Verify Tunnel Server

Check that the tunnel server is running:

```bash
# Check health endpoint
curl http://localhost/health

# Check tunnel server status
curl http://localhost/status

# Check metrics
curl http://localhost/metrics
```

## Part 2: Setting Up the Tunnel Client (Home Network)

### 2.1 Create Tunnel Client Configuration

Create `config.tunnel-client-home.toml` on your home machine:

```toml
# Home Tunnel Client Configuration
# This connects to the tunnel server and exposes local services

[logging]
level = "info"
file_logging = true
logs_directory = "./logs"
format = "text"
output_mode = "both"

[application]
name = "httpserver-tunnel-client"
environment = "development"

[server]
default_port = 3000
request_timeout = 30
enable_health_endpoints = true

# DISABLE SSL for this example
[server.ssl]
enabled = false

[static_config]
directory = "./website"
fallback = "index.html"

# TUNNEL CLIENT CONFIGURATION
[tunnel]
enabled = true

# Local port that will be tunneled (your local web server)
local_port = 3000

# Local host to bind to
local_host = "127.0.0.1"

# Tunnel server endpoint configuration
[[tunnel.endpoints]]
# WebSocket URL to your tunnel server (replace with your server's IP/domain)
server_url = "ws://YOUR_DROPLET_IP:8081/connect"

# Optional: request a specific subdomain
# subdomain = "myapp"

# Protocol version
protocol_version = "1.0"

# Connection settings
connection_timeout = 30
keepalive_interval = 30
max_connections = 100

# Authentication (must match server configuration)
[tunnel.auth]
method = "api_key"
api_key = "your-secret-api-key-here"

# Auto-reconnection settings
[tunnel.reconnection]
enabled = true
initial_delay = 1
max_delay = 300
backoff_multiplier = 2.0
max_attempts = 0  # Unlimited retries
jitter_factor = 0.1

# Monitoring
[tunnel.monitoring]
enabled = true
health_interval = 60
collect_metrics = true
log_events = true

# DISABLE SSL for tunnel connections
[tunnel.ssl]
verify_server = false

# Disable tunnel server (this is a client)
[tunnel.server]
enabled = false
```

### 2.2 Start Your Local Web Application

First, start your local web application on port 3000:

```bash
# Example: Start a simple static server on port 3000
./httpserver --port 3000 --directory ./my-website
```

### 2.3 Start the Tunnel Client

In a separate terminal, start the tunnel client:

```bash
# Start the tunnel client
./httpserver --config config.tunnel-client-home.toml
```

The client will:
- Connect to your tunnel server via WebSocket
- Authenticate using the API key
- Receive a subdomain assignment
- Forward incoming requests to your local server on port 3000

## Part 3: Testing the Tunnel Connection

### 3.1 Monitor Connection Status

On the tunnel client (home), check the logs for successful connection:

```
INFO tunnel client authenticated successfully
INFO assigned subdomain: abc12345
INFO tunnel ready for traffic on: abc12345.yourdomain.com
```

### 3.2 Test Public Access

From any internet-connected device, visit your assigned subdomain:

```
http://abc12345.yourdomain.com
```

This should display your local website content, proving the tunnel is working.

### 3.3 Verify Traffic Flow

Check logs on both servers:

**Tunnel Server (Digital Ocean):**
```
INFO received tunnel connection from: XXX.XXX.XXX.XXX
INFO assigned subdomain: abc12345
INFO forwarding request to tunnel client
```

**Tunnel Client (Home):**
```
INFO received HTTP request via tunnel: GET /
INFO forwarding to local server: http://127.0.0.1:3000
INFO sending response back through tunnel
```

## Part 4: Advanced Configuration

### 4.1 Custom Subdomain

To request a specific subdomain, modify the client config:

```toml
[[tunnel.endpoints]]
server_url = "ws://YOUR_DROPLET_IP:8081/connect"
subdomain = "myapp"  # Request specific subdomain
```

### 4.2 Multiple Tunnels

You can run multiple tunnel clients from the same home network:

```toml
# Client 1 - Main website
local_port = 3000
subdomain = "www"

# Client 2 - API server  
local_port = 3001
subdomain = "api"
```

### 4.3 Production Hardening

For production use, consider:

1. **Enable SSL/TLS** (modify both configs)
2. **Use JWT authentication** instead of API keys
3. **Implement rate limiting** per tunnel
4. **Set up monitoring** and alerting
5. **Configure firewall rules** on the droplet

## Part 5: Troubleshooting

### 5.1 Connection Issues

**Problem:** Client cannot connect to server
**Solution:** 
- Check firewall rules on Digital Ocean droplet (port 8081)
- Verify server URL in client config
- Check network connectivity

**Problem:** Authentication failed
**Solution:**
- Verify API key matches between client and server configs
- Check server logs for authentication errors

### 5.2 Traffic Issues

**Problem:** Public traffic not reaching local server
**Solution:**
- Verify local server is running on correct port
- Check tunnel client logs for forwarding errors
- Test local server directly: `curl http://localhost:3000`

**Problem:** Slow response times
**Solution:**
- Check network latency between client and server
- Adjust buffer sizes in performance settings
- Monitor bandwidth usage

### 5.3 Subdomain Issues

**Problem:** Cannot access assigned subdomain
**Solution:**
- Verify DNS records point to your Digital Ocean droplet
- Check subdomain assignment in client logs
- Test direct IP access first

## Part 6: Example Commands Summary

### Digital Ocean Droplet (Tunnel Server)

```bash
# Start tunnel server
./httpserver --config config.tunnel-server-production.toml --port 80

# Check status
curl http://localhost/health
curl http://localhost/status
```

### Home Machine (Tunnel Client)

```bash
# Start local web server
./httpserver --port 3000 --directory ./my-website

# Start tunnel client (separate terminal)
./httpserver --config config.tunnel-client-home.toml

# Check tunnel status
curl http://localhost:3000/health
```

### Testing

```bash
# Test public access (replace with your assigned subdomain)
curl http://abc12345.yourdomain.com

# Test with specific path
curl http://abc12345.yourdomain.com/api/status
```

## Conclusion

This setup creates a secure tunnel between your public Digital Ocean server and your local home server, allowing you to expose local services to the internet without opening firewall ports or configuring complex networking. The tunnel uses WebSocket connections for reliable communication and includes automatic reconnection and monitoring capabilities.

The tunnel system supports multiple clients, load balancing, authentication, and rate limiting, making it suitable for both development and production use cases.
