# 🚀 Deployment Guide - Rust HTTP Server

## Quick Start Deployment

### 1. Development Setup
```bash
# Clone and build
git clone <your-repo>
cd rusty_webserver
cargo build --release

# Test locally
./target/release/httpserver --directory ./public --port 8080
```

### 2. Production Deployment

#### Option A: Direct Binary Deployment
```bash
# Build optimized binary
cargo build --release

# Copy binary to server
scp target/release/httpserver user@server:/usr/local/bin/

# Run on server
ssh user@server "/usr/local/bin/httpserver --directory /var/www/html --port 80"
```

#### Option B: Cross-Platform Builds
```bash
# Build for multiple platforms
./build.sh

# Deploy appropriate binary
# For Linux servers:
scp dist/x86_64-unknown-linux-gnu/httpserver-linux-x64 user@server:/usr/local/bin/httpserver

# For ARM servers (like Raspberry Pi):
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

### 3. Systemd Service (Linux)

Create `/etc/systemd/system/httpserver.service`:
```ini
[Unit]
Description=Rust HTTP Server
After=network.target

[Service]
Type=simple
User=www-data
Group=www-data
WorkingDirectory=/var/www
ExecStart=/usr/local/bin/httpserver --directory /var/www/html --port 8080
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable httpserver
sudo systemctl start httpserver
sudo systemctl status httpserver
```

### 4. Docker Deployment

Create `Dockerfile`:
```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/httpserver /usr/local/bin/httpserver
EXPOSE 8080
CMD ["httpserver", "--directory", "/var/www", "--port", "8080"]
```

Build and run:
```bash
docker build -t httpserver .
docker run -p 8080:8080 -v /path/to/files:/var/www httpserver
```

### 5. Nginx Reverse Proxy

Configure Nginx as a reverse proxy:
```nginx
server {
    listen 80;
    server_name example.com;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### 6. Performance Tuning

#### System Limits
```bash
# Increase file descriptor limits
echo "* soft nofile 65536" >> /etc/security/limits.conf
echo "* hard nofile 65536" >> /etc/security/limits.conf
```

#### Server Configuration
```bash
# High-performance setup
./httpserver --directory /var/www/html --port 8080

# For high traffic, consider:
# - Multiple instances behind a load balancer
# - CDN for static assets
# - Compression middleware (if needed)
```

### 7. Monitoring

#### Health Check Script
```bash
#!/bin/bash
# health-check.sh
curl -f http://localhost:8080/ || exit 1
```

#### Log Monitoring
```bash
# Monitor server output
journalctl -u httpserver -f

# Or if running directly:
./httpserver 2>&1 | tee server.log
```

### 8. Security Considerations

#### Firewall Setup
```bash
# Allow only necessary ports
ufw allow 22    # SSH
ufw allow 80    # HTTP
ufw allow 443   # HTTPS (if using SSL termination)
ufw enable
```

#### User Permissions
```bash
# Create dedicated user
sudo adduser --system --group httpserver
sudo chown -R httpserver:httpserver /var/www/html

# Run with limited privileges
sudo -u httpserver ./httpserver --directory /var/www/html --port 8080
```

#### File Permissions
```bash
# Secure file permissions
find /var/www/html -type f -exec chmod 644 {} \;
find /var/www/html -type d -exec chmod 755 {} \;
```

### 9. Backup Strategy

```bash
#!/bin/bash
# backup.sh
DATE=$(date +%Y%m%d_%H%M%S)
tar -czf "/backup/website_$DATE.tar.gz" /var/www/html
find /backup -name "website_*.tar.gz" -mtime +30 -delete
```

### 10. Troubleshooting

#### Common Issues
```bash
# Port already in use
netstat -tulpn | grep :8080
kill -9 <PID>

# Permission denied
sudo chown $USER:$USER /path/to/files
chmod +r /path/to/files/*

# Check server status
curl -I http://localhost:8080/
```

#### Debug Mode
```bash
# Verbose logging (add to main.rs if needed)
RUST_LOG=debug ./httpserver --directory . --port 8080
```

## Environment-Specific Notes

### Windows Deployment
```cmd
# Windows Service (with nssm)
nssm install HttpServer "C:\path\to\httpserver.exe"
nssm set HttpServer Parameters "--directory C:\www --port 8080"
nssm start HttpServer
```

### macOS Deployment
```bash
# LaunchAgent for user service
# Create ~/Library/LaunchAgents/com.example.httpserver.plist
```

### Cloud Deployment

#### AWS EC2
```bash
# User data script
#!/bin/bash
yum update -y
wget https://github.com/yourrepo/releases/download/v1.0/httpserver-linux-x64
chmod +x httpserver-linux-x64
./httpserver-linux-x64 --directory /var/www/html --port 80
```

#### Google Cloud Run
```dockerfile
FROM gcr.io/distroless/cc-debian11
COPY httpserver /httpserver
EXPOSE 8080
CMD ["/httpserver", "--directory", "/var/www", "--port", "8080"]
```

Happy deploying! 🚀
