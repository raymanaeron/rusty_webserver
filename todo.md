# HTTP Server Gateway - Development Plan

## Project Overview
Transform the current static HTTP server into a powerful application gateway that supports:
- Static file serving (preserve existing functionality) ✅ **COMPLETED**
- Reverse proxy with load balancing
- Multiple backend service routing
- Health checks and failover
- Modular architecture for maintainability ✅ **COMPLETED**

## 🎯 Current Status: **Phase 3 Complete - WebSocket Support Added** ✅
**Last Updated**: January 3, 2025  
**Domain**: httpserver.io (acquired ✅)  
**Architecture**: Fully modularized Rust workspace  
**All existing functionality preserved**: ✅ Static file serving works perfectly  
**Configuration System**: ✅ TOML parsing and validation complete  
**Route Matching Engine**: ✅ Path-based routing with wildcards implemented  
**HTTP Proxy**: ✅ Complete request forwarding with streaming response handling  
**Load Balancing**: ✅ All 4 strategies implemented with comprehensive testing  
**Target Management**: ✅ Complete target pool management with health tracking  
**Configuration Schema**: ✅ Full multi-target configuration support  
**WebSocket Support**: ✅ WebSocket detection, proxying, and load balancing implemented
**Test Organization**: ✅ All tests extracted into separate files by functionality

## 📊 Test Organization Standards

### **Test Separation Requirement**
All crates must follow standardized test organization:
- **No embedded tests** in `src/lib.rs` files
- **Separate test files** in `tests/` directory 
- **Group by functionality** - organize tests by intent/category, not one test per file
- **Clear naming** - test file names should indicate the functionality being tested
- **Public API only** - tests should only use public APIs, no private struct/method access

### **Current Test Structure (26 total tests):**
```
httpserver-balancer/tests/           (12 tests in 4 files)
├── load_balancing_strategies.rs     - 4 tests: All strategy algorithms
├── target_management.rs            - 4 tests: Health, empty targets, single target  
├── connection_tracking.rs           - 1 test: Connection increment/decrement
└── utilities.rs                     - 3 tests: GCD, serialization

httpserver-proxy/tests/              (14 tests in 3 files)
├── route_matching.rs                - 6 tests: Path matching, wildcards, priority
├── proxy_handler.rs                 - 2 tests: Handler integration
└── websocket_support.rs             - 6 tests: WebSocket detection, routing, load balancing
```  

## 📋 Development Log & Session Context

### ✅ **Phase 1.1 Complete (Current Session)**
**What was done:**
- Successfully split monolithic `main.rs` into 5 focused library crates
- Created clean workspace structure with shared dependencies
- Preserved 100% of existing functionality (static file serving with SPA fallback)
- All build scripts (`b_mac.sh`, `b_linux.sh`, `b_win.bat`) work unchanged
- All clean scripts (`c_mac.sh`, `c_linux.sh`, `c_win.bat`) work unchanged
- Comprehensive testing verified debug and release builds compile and run correctly
- Created detailed 6-phase roadmap with business model and technical architecture

**Current file structure:**
```
rusty_webserver/
├── Cargo.toml (workspace)
├── todo.md (this file)
├── httpserver/src/main.rs (main binary)
├── httpserver-core/src/lib.rs (server startup, middleware, logging)
├── httpserver-static/src/lib.rs (static file serving)
├── httpserver-config/src/lib.rs (CLI parsing, config structs)
├── httpserver-proxy/src/lib.rs (placeholder for Phase 2)
├── httpserver-balancer/src/lib.rs (placeholder for Phase 3)
└── build/clean scripts (all working)
```

### ✅ **Phase 1.2 Complete (Current Session)**
**What was done:**
- Implemented complete TOML configuration file parsing using `serde` and `toml` crates
- Added robust configuration validation with detailed error messages
- Created comprehensive config validation for static directories and proxy routes (future)
- Added example configuration files: `config.simple.toml`, `config.example.toml`
- Tested configuration loading end-to-end with validation
- Preserved 100% backward compatibility - CLI arguments still work without config files
- Enhanced error handling with descriptive messages for configuration issues

**Configuration features implemented:**
- TOML file parsing with proper error handling
- Static file configuration validation (directory existence, fallback file)
- Proxy route configuration validation (URL format, timeout values) - ready for Phase 2
- CLI argument override capability (config file + CLI args work together)
- Multiple example configuration files for different use cases

**Testing completed:**
- Verified config file loading works correctly
- Validated error handling for invalid directories
- Confirmed CLI arguments override config file settings
- Tested TOML parsing error messages are clear and helpful

### ✅ **Phase 2.1 Complete (Current Session)**
**What was done:**
- Implemented comprehensive route matching engine for reverse proxy functionality
- Created `RouteMatcher` with support for exact paths and wildcard patterns (`/api/*`)
- Added priority system where proxy routes take precedence over static files
- Implemented path manipulation with automatic path stripping for wildcard routes
- Added support for global wildcard (`*`) and prefix matching (`/api/*`)
- Created `RouteMatch` structure to return matched route info and stripped paths
- Integrated proxy route detection into main server startup
- Added comprehensive unit tests (8 tests, all passing) covering:
  - Exact path matching (`/health`)
  - Wildcard path matching (`/api/*`)
  - Route priority (first match wins)
  - Path normalization (handles with/without leading slash)
  - Global wildcard matching (`*`)
  - Pattern compilation logic
  - Empty routes handling
  - Full ProxyHandler integration

**Route matching features implemented:**
- ✅ Path-based routing (`/api/*`, `/admin/*`, `/health`)
- ✅ Priority system (proxy routes processed before static files)
- ✅ Path manipulation (automatic stripping for forwarding)
- ✅ Wildcard support (`/*` and `*` patterns)
- ✅ Exact match support for specific endpoints
- ✅ Path normalization (leading slash handling)
- ✅ Order-based precedence (first matching route wins)

**Testing completed:**
- ✅ All 8 unit tests pass covering edge cases and functionality
- ✅ Configuration loading works with proxy routes
- ✅ Static file serving preserved when no proxy routes configured
- ✅ Route detection and logging working correctly

**Next development session should focus on:**
1. **Phase 2.2**: Implement HTTP proxy forwarding functionality
2. Add request forwarding to target servers using `reqwest`
3. Implement response streaming back to clients
4. Add proper header handling (Host, X-Forwarded-For, etc.)

## Phase 1: Architecture & Foundation

### 1.1 Project Modularization ✅ **COMPLETED**
- [x] **Create library structure** - Split monolithic `main.rs` into focused libraries
  - [x] `httpserver-core` - Core server functionality, middleware, startup logic
  - [x] `httpserver-static` - Static file serving logic (extracted from original main.rs)
  - [x] `httpserver-proxy` - Reverse proxy implementation (placeholder created)
  - [x] `httpserver-balancer` - Load balancing strategies (placeholder created)
  - [x] `httpserver-config` - Configuration parsing and CLI argument handling
- [x] **Refactor main.rs** - Moved to `httpserver/src/main.rs`, now only orchestrates libraries
- [x] **Add workspace Cargo.toml** - Root workspace managing 5 crates + main binary
- [x] **Update build scripts** - All platform build scripts (`b_*.sh`, `b_win.bat`) work unchanged
- [x] **Test organization** - Extract tests into separate files by functionality ✅ COMPLETED

### 1.2 Configuration System ✅ **COMPLETED**
- [x] **Design configuration schema** - Config structs with serde support for future TOML
- [x] **CLI argument structure** - Extended with `--config` parameter for future proxy config
- [x] **Add CLI argument** - `--config` parameter ready for proxy configuration file
- [x] **Implement config parsing** - Use `serde` and `toml` for configuration (ready to implement)
- [x] **Configuration validation** - Validate routes, targets, and settings on startup
- [x] **Default configuration** - Provide sensible defaults and example config
- [x] **Test organization** - Separate test files for configuration functionality ✅ COMPLETED

### 1.3 Dependencies & Setup ✅ **COMPLETED**
- [x] **Add new dependencies** to workspace `Cargo.toml`:
  - [x] `reqwest = { version = "0.11", features = ["json", "stream"] }` - Ready for Phase 2
  - [x] `serde = { version = "1.0", features = ["derive"] }` - Configuration serialization
  - [x] `toml = "0.8"` - Configuration file parsing (ready for Phase 2)
  - [x] `tokio = { version = "1.0", features = ["full", "sync"] }` - Async runtime
- [x] **Update existing imports** - Organized by library modules, clean separation
- [x] **Test organization** - Establish standardized test structure for all crates ✅ COMPLETED

## Phase 2: Basic Reverse Proxy

### 2.1 Proxy Route Matching ✅ **COMPLETED**
- [x] **Route matching engine** - Implement path-based routing (`/api/*`, `/admin/*`)
- [x] **Priority system** - Proxy routes take precedence over static files
- [x] **Path manipulation** - Support for path stripping/rewriting
- [x] **Wildcard support** - Handle `/*` and specific path patterns
- [x] **Test organization** - Separate test files for route matching functionality ✅ COMPLETED

### 2.2 HTTP Proxy Implementation ✅ **COMPLETED**
- [x] **Request forwarding** - Forward HTTP method, headers, and body to target
- [x] **Response streaming** - Stream response back to client efficiently
- [x] **Header handling** - Preserve/modify headers (Host, X-Forwarded-For, etc.)
- [x] **Error handling** - Handle target unreachable, timeouts, connection errors
- [x] **Timeout configuration** - Configurable request timeouts per route
- [x] **Test organization** - Separate test files for proxy handler functionality ✅ COMPLETED

### 2.3 Router Integration ✅ **COMPLETED**
- [x] **Modify Axum router** - Add proxy routes before static file routes
- [x] **Middleware compatibility** - Ensure logging and CORS work with proxy
- [x] **Request extraction** - Extract full request for forwarding
- [x] **Response conversion** - Convert proxy responses to Axum responses
- [x] **Test organization** - Test coverage for router integration ✅ COMPLETED

## Phase 3: Load Balancing

### 3.1 Load Balancing Strategies ✅ **COMPLETED**
- [x] **Round Robin** - Simple sequential target selection ✅ TESTED
- [x] **Weighted Round Robin** - Support for target weights with GCD-based algorithm ✅ TESTED
- [x] **Random** - Random target selection with time-based seeding ✅ TESTED
- [x] **Least Connections** - Track and route to least busy target ✅ TESTED
- [x] **Strategy configuration** - Per-route load balancing strategy selection ✅ TESTED
- [x] **Comprehensive testing** - 11 unit tests covering all strategies and edge cases ✅ PASSED
- [x] **End-to-end validation** - Perfect round-robin distribution verified ✅ TESTED
- [x] **Backward compatibility** - Single-target routes still work ✅ VERIFIED
- [x] **Thread safety** - Arc<Mutex<>> for shared state management ✅ IMPLEMENTED
- [x] **All compilation errors fixed** - Test assertions updated for Option<String> ✅ FIXED
- [x] **Test organization** - Separate test files for load balancing functionality ✅ COMPLETED

### 3.2 Target Management ✅ **COMPLETED**
- [x] **Target pool** - Manage multiple targets per route ✅ IMPLEMENTED
- [x] **Connection tracking** - Track active connections per target ✅ IMPLEMENTED
- [x] **State management** - Thread-safe counters and target state ✅ IMPLEMENTED
- [x] **Target selection** - Implement selection algorithms ✅ IMPLEMENTED
- [x] **Fallback logic** - Handle when all targets are unavailable ✅ IMPLEMENTED
- [x] **Test organization** - Separate test files for target management functionality ✅ COMPLETED

### 3.3 Configuration Schema ✅ **COMPLETED**
- [x] **Multi-target config** - Support arrays of targets in configuration ✅ IMPLEMENTED
- [x] **Weight configuration** - Target weights for weighted strategies ✅ IMPLEMENTED
- [x] **Strategy per route** - Different strategies for different routes ✅ IMPLEMENTED
- [x] **Global defaults** - Default strategy and settings ✅ IMPLEMENTED
- [x] **Test organization** - Separate test files for configuration schema functionality ✅ COMPLETED

### 3.4 WebSocket Support ✅ **PARTIALLY COMPLETED**
- [x] **WebSocket detection** - Detect WebSocket upgrade requests (`Upgrade: websocket`) ✅ IMPLEMENTED
- [x] **WebSocket proxying** - Proxy WebSocket connections to backends ✅ IMPLEMENTED
- [x] **Connection management** - Handle WebSocket connection lifecycle ✅ IMPLEMENTED
- [ ] **Sticky sessions** - Route WebSocket connections to same backend ❌ NOT IMPLEMENTED
- [x] **Load balancing for WebSockets** - Handle persistent connections in load balancing ✅ IMPLEMENTED
- [ ] **WebSocket health checks** - Verify WebSocket endpoints are healthy ❌ NOT IMPLEMENTED (requires Phase 4)
- [x] **Test organization** - Separate test files for WebSocket functionality ✅ COMPLETED

## Phase 4: Health Checks & Monitoring

### 4.1 Health Check System
- [ ] **Health check endpoints** - Configurable health check paths (`/health`, `/ping`)
- [ ] **Background health checks** - Periodic health monitoring task
- [ ] **Health status tracking** - Track healthy/unhealthy targets
- [ ] **Automatic recovery** - Re-add targets when they become healthy
- [ ] **Configurable intervals** - Health check frequency per route
- [ ] **Test organization** - Separate test files for health check functionality

### 4.2 Circuit Breaker Pattern
- [ ] **Failure tracking** - Track consecutive failures per target
- [ ] **Circuit states** - Closed, Open, Half-Open states
- [ ] **Failure thresholds** - Configurable failure limits
- [ ] **Recovery testing** - Half-open state for testing recovery
- [ ] **Timeout configuration** - Circuit breaker timeout settings
- [ ] **Test organization** - Separate test files for circuit breaker functionality

### 4.3 Enhanced Logging
- [ ] **Proxy request logging** - Log proxy requests with target info
- [ ] **Health check logging** - Log health check results
- [ ] **Performance metrics** - Response times, error rates
- [ ] **Load balancer stats** - Target selection and distribution stats
- [ ] **WebSocket logging** - Log WebSocket connections and upgrades
- [ ] **Test organization** - Separate test files for logging functionality

### 4.4 SSL/TLS Support
- [ ] **SSL termination** - Handle HTTPS at the gateway, forward HTTP to backends
- [ ] **Certificate management** - Load cert/key files from filesystem
- [ ] **HTTP to HTTPS redirect** - Automatic redirect for SSL-enabled routes
- [ ] **Backend SSL support** - Option to forward HTTPS to backends
- [ ] **Let's Encrypt integration** - Automatic certificate generation/renewal
- [ ] **SSL configuration** - Per-route SSL settings and certificate selection

## Phase 5: Advanced Features

### 5.1 Request/Response Middleware
- [ ] **Header injection** - Add custom headers to requests/responses
- [ ] **Request modification** - Modify requests before forwarding
- [ ] **Response modification** - Modify responses before returning
- [ ] **Authentication headers** - Add auth headers for backend services
- [ ] **Rate limiting** - Per-route and per-client rate limiting
- [ ] **Request/response compression** - Gzip/Brotli compression support
- [ ] **Test organization** - Separate test files for middleware functionality

### 5.2 Advanced SSL Features
- [ ] **SNI support** - Server Name Indication for multiple domains
- [ ] **SSL passthrough** - Forward encrypted traffic without termination
- [ ] **Certificate hot-reload** - Reload certificates without restart
- [ ] **OCSP stapling** - Online Certificate Status Protocol
- [ ] **TLS 1.3 support** - Latest TLS protocol support
- [ ] **Test organization** - Separate test files for SSL functionality

### 5.3 Enterprise Features
- [ ] **Metrics endpoint** - Prometheus-compatible metrics
- [ ] **Admin API** - REST API for configuration and monitoring
- [ ] **Configuration hot-reload** - Update config without restart
- [ ] **Access logging** - Structured logging in various formats
- [ ] **Request tracing** - Distributed tracing support
- [ ] **Test organization** - Separate test files for enterprise features

## Phase 6: Public Tunnel Service (Revolutionary Feature)

### 6.1 Tunnel Client (Local HTTP Server)
- [ ] **Tunnel client integration** - Built-in tunnel client in local HTTP server
- [ ] **Secure WebSocket connection** - Encrypted tunnel to public server
- [ ] **Authentication system** - API keys, user accounts, subdomain management
- [ ] **Auto-reconnection** - Handle network interruptions gracefully
- [ ] **Tunnel status monitoring** - Show tunnel health and public URL
- [ ] **Multiple tunnel support** - Support multiple public URLs per local server
- [ ] **Test organization** - Separate test files for tunnel client functionality

### 6.2 Public Tunnel Server (httpserver.io)
- [ ] **Tunnel server architecture** - Separate server for handling public traffic
- [ ] **Subdomain management** - Dynamic subdomain allocation (`abc123.httpserver.io`)
- [ ] **Custom domain support** - Allow custom domains (`api.mycompany.com`)
- [ ] **SSL certificate automation** - Auto-generate SSL certs for subdomains
- [ ] **User management** - Account creation, API key management
- [ ] **Traffic routing** - Route public requests to correct tunnel connections
- [ ] **Rate limiting** - Prevent abuse on public endpoints
- [ ] **Test organization** - Separate test files for tunnel server functionality

### 6.3 Tunnel Protocol Implementation
- [ ] **Bidirectional communication** - WebSocket-based tunnel protocol
- [ ] **Request forwarding** - Forward HTTP requests through tunnel
- [ ] **Response streaming** - Stream responses back through tunnel
- [ ] **Connection multiplexing** - Multiple HTTP requests over single tunnel
- [ ] **Compression** - Compress tunnel traffic for performance
- [ ] **Protocol versioning** - Support protocol upgrades
- [ ] **Test organization** - Separate test files for tunnel protocol functionality

### 6.4 Security & Authentication
- [ ] **TLS everywhere** - Encrypt all tunnel traffic
- [ ] **API key authentication** - Secure tunnel establishment
- [ ] **Request validation** - Validate incoming public requests
- [ ] **Access control** - IP whitelisting, geographic restrictions
- [ ] **Audit logging** - Log all public traffic and tunnel activity
- [ ] **DDoS protection** - Rate limiting and traffic filtering
- [ ] **Test organization** - Separate test files for security functionality

### 6.5 Management & Monitoring
- [ ] **Web dashboard** - Manage tunnels, view traffic, analytics
- [ ] **Real-time analytics** - Request counts, response times, error rates
- [ ] **Tunnel logs** - View requests coming through public URLs
- [ ] **Bandwidth monitoring** - Track data usage per tunnel
- [ ] **Alert system** - Notifications for tunnel issues
- [ ] **CLI management** - Command-line tools for tunnel management
- [ ] **Test organization** - Separate test files for management functionality

### 6.6 Deployment Infrastructure
- [ ] **Docker containers** - Containerized tunnel server deployment
- [ ] **Load balancer support** - Multiple tunnel server instances
- [ ] **Database integration** - Store user accounts, tunnels, analytics
- [ ] **CDN integration** - Global edge locations for performance
- [ ] **Monitoring & alerting** - Server health monitoring
- [ ] **Auto-scaling** - Handle traffic spikes automatically
- [ ] **Test organization** - Separate test files for deployment functionality

## Configuration Examples

### Basic Configuration
```toml
# Static files (default behavior - no change needed)
[static]
directory = "./public"
fallback = "index.html"

# Simple proxy
[[proxy]]
path = "/api/*"
target = "http://localhost:3000"
timeout = 30
```

### Load Balanced Configuration
```toml
[[proxy]]
path = "/api/*"
strategy = "round_robin"
targets = [
    "http://localhost:3000",
    "http://localhost:3001",
    "http://localhost:3002"
]
health_check = "/health"
health_interval = 30
```

### HTTPS Configuration
```toml
[ssl]
enabled = true
cert_file = "./certs/server.crt"
key_file = "./certs/server.key"
redirect_http = true

[[proxy]]
path = "/api/*"
targets = ["http://localhost:3000"]  # Backend stays HTTP
ssl_required = true  # Only serve over HTTPS
```

### WebSocket Configuration
```toml
[[proxy]]
path = "/ws/*"
targets = ["http://localhost:3001"]
protocol = "websocket"
sticky_sessions = true  # Keep connections to same backend
health_check = "/ws/ping"
```

### Tunnel Configuration
```toml
[tunnel]
enabled = true
server = "tunnel.httpserver.io"
api_key = "your-api-key-here"
subdomain = "myproject"  # Results in https://myproject.httpserver.io
custom_domain = "api.mycompany.com"  # Optional custom domain

# Expose specific local services publicly
[[tunnel.expose]]
local_path = "/api/*"
public_path = "/api/*"
local_target = "http://localhost:3000"

[[tunnel.expose]]
local_path = "/admin/*"
public_path = "/admin/*"
local_target = "http://localhost:8000"
```

### Complete Integration Example
```toml
# Local static files
[static]
directory = "./public"

# Local proxy routes
[[proxy]]
path = "/api/*"
targets = ["http://localhost:3000", "http://localhost:3001"]
strategy = "round_robin"

# Public tunnel
[tunnel]
enabled = true
subdomain = "myapp"
expose_all = true  # Expose all local routes publicly
```

## Success Criteria

### Phase 1 Complete
- [x] Modular project structure
- [x] Configuration system working
- [x] All existing functionality preserved

### Phase 2 Complete
- [ ] Basic proxy functionality working
- [ ] Single target per route supported
- [ ] Logging includes proxy requests

### Phase 3 Complete
- [ ] Multiple targets per route
- [ ] Load balancing strategies implemented
- [ ] Configuration supports multi-target setup
- [ ] WebSocket proxying operational

### Phase 4 Complete
- [ ] Health checks operational
- [ ] Automatic target removal/addition
- [ ] Circuit breaker pattern implemented
- [ ] SSL termination working

### Phase 5 Complete
- [ ] Production-ready features
- [ ] Advanced SSL features
- [ ] Enterprise monitoring and management

### Phase 6 Complete
- [ ] Tunnel client integrated into local server
- [ ] Public tunnel server deployed and operational
- [ ] Secure tunnel protocol implemented
- [ ] Web dashboard for tunnel management
- [ ] Custom domain support working
- [ ] Production-ready tunnel service

## Testing Strategy
- [ ] **Test organization** - All tests must be in separate `tests/` files, grouped by functionality ✅ **COMPLETED for Phases 1-3**
- [ ] **Unit tests** - Test individual components and strategies
- [ ] **Integration tests** - Test proxy with real backend services
- [ ] **WebSocket testing** - Test WebSocket proxying with real WebSocket servers
- [ ] **SSL testing** - Test HTTPS termination and certificate handling
- [ ] **Tunnel testing** - Test tunnel establishment and traffic forwarding
- [ ] **End-to-end testing** - Test complete local-to-public tunnel flow
- [ ] **Load testing** - Verify performance under load (local and tunnel)
- [ ] **Security testing** - Penetration testing of tunnel infrastructure
- [ ] **Failover testing** - Test health checks and circuit breakers
- [ ] **Cross-platform testing** - Verify all platforms work correctly

## Business Model & Monetization
- [ ] **Free tier** - Limited tunnels, subdomains, bandwidth
- [ ] **Pro tier** - Custom domains, higher limits, analytics
- [ ] **Enterprise tier** - White-label, dedicated infrastructure, SLA
- [ ] **Open source core** - Local HTTP server remains open source
- [ ] **SaaS tunnel service** - Hosted tunnel infrastructure as paid service
