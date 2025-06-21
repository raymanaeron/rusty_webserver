# HTTP Server UI Implementation Plan

## Architecture Overview

```
┌─────────────────────┐    ┌─────────────────────┐    ┌─────────────────────┐
│   httpserver-cli    │    │   httpserver-ui     │    │  httpserver-engine  │
│   (bin: httpserver) │    │ (bin: httpserver-ui)│    │      (lib crate)    │
│                     │    │                     │    │                     │
│  ┌───────────────┐  │    │  ┌───────────────┐  │    │  ┌───────────────┐  │
│  │ Direct Engine │──┼────┼──│      Wry      │  │    │  │  Core Engine  │  │
│  │   API Calls   │  │    │  │   WebView UI  │  │    │  │   Business    │  │
│  └───────────────┘  │    │  └───────────────┘  │    │  │     Logic     │  │
└─────────────────────┘    │           │         │    │  └───────────────┘  │
                           │           │         │    │                     │
                           │  ┌───────────────┐  │    │  ┌───────────────┐  │
                           │  │ HTTP Client   │  │    │  │   HTTP API    │  │
                           │  │   (REST)      │  │    │  │   Server      │  │
                           │  └───────────────┘  │    │  │  Port: 9812   │  │
                           │           │         │    │  └───────────────┘  │
                           │  ┌───────────────┐  │    │                     │
                           │  │  WebSocket    │  │    │  ┌───────────────┐  │
                           │  │    Client     │  │    │  │   WebSocket   │  │
                           │  └───────────────┘  │    │  │    Server     │  │
                           └─────────────────────┘    │  │  Port: 9852   │  │
                                                      │  └───────────────┘  │
┌─────────────────────────────────────────────────────┤                     │
│             httpserver-api                          │  ┌───────────────┐  │
│            (HTTP API Layer)                         │  │      FFI      │  │
│                                                     │  │    Layer      │  │
│  ┌─────────────────┐  ┌─────────────────┐           │  │  (Future)     │  │
│  │   REST API      │  │   WebSocket     │           │  └───────────────┘  │
│  │   Endpoints     │  │   Real-time     │           └─────────────────────┘
│  │                 │  │    Updates      │          
│  │ • Server Status │  │                 │          
│  │ • Config CRUD   │  │ • Health Stats  │          
│  │ • SSL Mgmt      │  │ • Log Streaming │          
│  │ • Auth/Tokens   │  │ • Server Events │          
│  └─────────────────┘  └─────────────────┘          
└─────────────────────────────────────────────────────┘
```

## Phase 1: Project Structure Refactoring

### 1.1 Rename and Restructure
- [ ] Rename `httpserver` → `httpserver-engine`
- [ ] Convert `httpserver-engine` from binary to library crate
- [ ] Create new workspace structure:
  ```
  Cargo.toml (workspace)
  ├── httpserver-engine/ (lib)
  ├── httpserver-cli/ (bin: httpserver)
  ├── httpserver-api/ (lib + bin)
  └── httpserver-ui/ (bin: httpserver-ui)
  ```

### 1.2 Extract Core Logic
- [ ] Move all main.rs logic into `httpserver-engine/src/lib.rs`
- [ ] Create public API for engine initialization
- [ ] Implement engine lifecycle management (start, stop, restart)
- [ ] Add engine state management and querying

## Phase 2: HTTP API Layer (httpserver-api)

### 2.1 API Infrastructure
- [ ] Create HTTP API server on port `9812`
- [ ] Implement JWT authentication with apikey/apisecret
- [ ] Add token manager with 1-minute token lifetime
- [ ] Create automatic token refresh mechanism
- [ ] Add CORS configuration for UI access

### 2.2 Authentication System
```toml
# app_config.toml
[api]
enabled = true
port = 9812
api_key = "your-api-key-here"
api_secret = "your-api-secret-here"
token_lifetime_seconds = 60
cors_origins = ["http://localhost:3000", "tauri://localhost"]
```

### 2.3 REST API Endpoints
- [ ] **Authentication**
  - `POST /api/v1/auth/login` - Get JWT token
  - `POST /api/v1/auth/refresh` - Refresh JWT token
  - `POST /api/v1/auth/logout` - Invalidate token

- [ ] **Server Management**
  - `GET /api/v1/server/status` - Get server status
  - `POST /api/v1/server/start` - Start server
  - `POST /api/v1/server/stop` - Stop server
  - `POST /api/v1/server/restart` - Restart server

- [ ] **Configuration Management**
  - `GET /api/v1/config` - Get current configuration
  - `PUT /api/v1/config` - Update configuration
  - `POST /api/v1/config/validate` - Validate configuration
  - `GET /api/v1/config/templates` - Get config templates

- [ ] **SSL Certificate Management**
  - `GET /api/v1/ssl/certificates` - List certificates
  - `POST /api/v1/ssl/certificates` - Upload certificate
  - `DELETE /api/v1/ssl/certificates/:id` - Delete certificate
  - `POST /api/v1/ssl/renew/:id` - Renew certificate
  - `POST /api/v1/ssl/letsencrypt` - Generate Let's Encrypt cert

- [ ] **Health & Monitoring**
  - `GET /api/v1/health` - API health check
  - `GET /api/v1/health/engine` - Engine health status
  - `GET /api/v1/metrics` - Server metrics
  - `GET /api/v1/logs` - Get recent logs

- [ ] **Static File Management**
  - `GET /api/v1/static/files` - List static files
  - `POST /api/v1/static/files` - Upload static file
  - `DELETE /api/v1/static/files/:path` - Delete static file
  - `PUT /api/v1/static/settings` - Update static settings

- [ ] **Proxy Management**
  - `GET /api/v1/proxy/routes` - List proxy routes
  - `POST /api/v1/proxy/routes` - Add proxy route
  - `PUT /api/v1/proxy/routes/:id` - Update proxy route
  - `DELETE /api/v1/proxy/routes/:id` - Delete proxy route

- [ ] **Tunnel Management**
  - `GET /api/v1/tunnel/status` - Tunnel status
  - `POST /api/v1/tunnel/start` - Start tunnel
  - `POST /api/v1/tunnel/stop` - Stop tunnel
  - `GET /api/v1/tunnel/endpoints` - List tunnel endpoints

### 2.4 WebSocket Real-time Updates (Port 9852)
- [ ] **Connection Management**
  - JWT authentication for WebSocket connections
  - Connection lifecycle management
  - Client subscription management

- [ ] **Real-time Events**
  - Server status changes
  - Health metric updates
  - Log streaming
  - Configuration changes
  - SSL certificate events
  - Traffic statistics
  - Error notifications

- [ ] **Event Types**
  ```rust
  enum WSEvent {
      ServerStatus(ServerStatusEvent),
      HealthMetrics(HealthMetricsEvent),
      LogEntry(LogEntryEvent),
      ConfigChanged(ConfigChangedEvent),
      SSLEvent(SSLEvent),
      TrafficStats(TrafficStatsEvent),
      Error(ErrorEvent),
  }
  ```

## Phase 3: CLI Implementation (httpserver-cli)

### 3.1 CLI Structure
- [ ] Create thin CLI wrapper around engine
- [ ] Implement direct engine API calls (no HTTP overhead)
- [ ] Maintain all existing CLI functionality
- [ ] Add new management commands:
  - `httpserver api start` - Start API server
  - `httpserver api stop` - Stop API server
  - `httpserver ui` - Launch UI (if installed)

### 3.2 CLI Commands
```bash
# Existing functionality
httpserver --config config.toml --port 8080
httpserver -d ./website --port 3000

# New management commands
httpserver api start --port 9812
httpserver api stop
httpserver ui launch
httpserver status
httpserver restart
```

## Phase 4: Web UI Implementation (httpserver-ui)

### 4.1 Technology Stack
- [ ] **Backend**: Rust + Wry for WebView
- [ ] **Frontend**: React/Vue.js + TypeScript
- [ ] **Styling**: Tailwind CSS or similar
- [ ] **Real-time**: WebSocket client
- [ ] **HTTP Client**: Axios or Fetch API

### 4.2 UI Features

#### 4.2.1 Dashboard
- [ ] Server status overview
- [ ] Real-time health metrics
- [ ] Traffic statistics
- [ ] Active connections
- [ ] System resource usage

#### 4.2.2 Configuration Management
- [ ] Visual configuration editor
- [ ] Configuration validation
- [ ] Template selection
- [ ] Export/Import configuration
- [ ] Configuration history/rollback

#### 4.2.3 SSL Certificate Management
- [ ] Certificate list with expiration dates
- [ ] Certificate upload interface
- [ ] Let's Encrypt integration
- [ ] Certificate renewal management
- [ ] Certificate validation tools

#### 4.2.4 Static File Management
- [ ] File browser interface
- [ ] File upload/download
- [ ] File type filtering
- [ ] Enable/disable file types
- [ ] Directory management

#### 4.2.5 Proxy Management
- [ ] Visual proxy route editor
- [ ] Load balancing configuration
- [ ] Health check configuration
- [ ] Route testing tools

#### 4.2.6 Tunnel Management
- [ ] Tunnel endpoint configuration
- [ ] Tunnel status monitoring
- [ ] Subdomain management
- [ ] Authentication configuration

#### 4.2.7 Monitoring & Logs
- [ ] Real-time log viewer
- [ ] Log filtering and search
- [ ] Health metrics charts
- [ ] Performance monitoring
- [ ] Alert configuration

#### 4.2.8 Authentication & Security
- [ ] Login interface
- [ ] Session management
- [ ] User preferences
- [ ] Security settings

### 4.3 UI Architecture
```
httpserver-ui/
├── src/
│   ├── main.rs                 # Wry application entry point
│   ├── webview/
│   │   ├── mod.rs             # WebView setup and management
│   │   └── handlers.rs        # Native function handlers
│   └── api/
│       ├── client.rs          # HTTP API client
│       └── websocket.rs       # WebSocket client
├── web/                       # Frontend web assets
│   ├── src/
│   │   ├── components/        # React/Vue components
│   │   ├── pages/            # Page components
│   │   ├── stores/           # State management
│   │   ├── api/              # API client code
│   │   └── utils/            # Utility functions
│   ├── public/               # Static assets
│   └── package.json          # Frontend dependencies
└── Cargo.toml
```

## Phase 5: Future Enhancements

### 5.1 FFI Layer
- [ ] Create C-compatible FFI interface
- [ ] Generate C header files
- [ ] Mobile SDK preparation

### 5.2 Advanced Features
- [ ] Database integration for configuration
- [ ] Advanced authentication (OAuth, LDAP)
- [ ] Plugin system
- [ ] Multi-server management
- [ ] Backup and restore
- [ ] Performance profiling tools

## Implementation Timeline

### Sprint 1 (Week 1-2): Foundation
- [ ] Project restructuring
- [ ] Engine library creation
- [ ] Basic CLI implementation

### Sprint 2 (Week 3-4): API Layer
- [ ] HTTP API infrastructure
- [ ] Authentication system
- [ ] Core API endpoints

### Sprint 3 (Week 5-6): WebSocket & Real-time
- [ ] WebSocket server implementation
- [ ] Real-time event system
- [ ] API completion

### Sprint 4 (Week 7-8): UI Foundation
- [ ] Wry application setup
- [ ] Frontend framework setup
- [ ] Basic UI components

### Sprint 5 (Week 9-10): UI Features
- [ ] Dashboard implementation
- [ ] Configuration management
- [ ] SSL management

### Sprint 6 (Week 11-12): Advanced UI
- [ ] Monitoring and logs
- [ ] Proxy and tunnel management
- [ ] Testing and polish

## Technical Considerations

### Security
- [ ] Secure token storage
- [ ] HTTPS enforcement for API
- [ ] Input validation and sanitization
- [ ] Rate limiting
- [ ] Audit logging

### Performance
- [ ] Efficient WebSocket message handling
- [ ] Optimized API responses
- [ ] Frontend bundle optimization
- [ ] Memory management

### Error Handling
- [ ] Comprehensive error types
- [ ] User-friendly error messages
- [ ] Error recovery mechanisms
- [ ] Logging and debugging

### Testing
- [ ] Unit tests for all components
- [ ] Integration tests for API
- [ ] E2E tests for UI
- [ ] Performance tests

## Notes

- Keep backward compatibility with existing CLI usage
- Ensure UI can be disabled/optional for headless deployments
- Plan for future mobile SDK integration
- Consider Docker containerization for UI deployment
- Document all API endpoints with OpenAPI/Swagger
