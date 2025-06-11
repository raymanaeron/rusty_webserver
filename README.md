# httpserver

A fast, lightweight, and cross-platform static file HTTP server built with Rust and Axum. Perfect for serving static websites, development environments, single-page applications, and local file sharing.

## Why httpserver?

- **Zero Configuration** - Works out of the box with sensible defaults
- **Cross-Platform** - Single binary runs on macOS, Linux, and Windows  
- **Fast & Lightweight** - Built with Rust for maximum performance and minimal resource usage
- **Developer Friendly** - Perfect for local development, testing, and quick file sharing
- **SPA Ready** - Built-in support for single-page applications with fallback routing
- **Secure by Default** - Includes directory traversal protection and CORS support

## Quick Start

Run with default settings (serves current directory on port 8080):
```bash
cargo run
```

Or download a pre-built binary and run:
```bash
./httpserver
```

Visit `http://localhost:8080` to see your files served.

## Installation

### From Source
```bash
# Clone the repository
git clone <repository-url>
cd rusty_webserver

# Build the project
cargo build --release

# The binary will be at target/release/httpserver
```

### Pre-built Binaries
Check the releases page for pre-built binaries for your platform.

## Usage

### Basic Commands

**Serve current directory on default port (8080):**
```bash
httpserver
```

**Serve specific directory:**
```bash
httpserver --directory /path/to/your/files
```

**Use custom port:**
```bash
httpserver --port 3000
```

**Combine options:**
```bash
httpserver --directory ./dist --port 8080
```

### Command Line Options

```
Usage: httpserver [OPTIONS]

Options:
  -d, --directory <DIRECTORY>  Directory to serve files from [default: .]
  -p, --port <PORT>            Port to listen on [default: 8080]
  -h, --help                   Print help
```

## Features

### File Serving
- **Smart MIME Detection** - Automatically serves files with correct Content-Type headers
- **Comprehensive File Support** - HTML, CSS, JS, images, fonts, documents, and more
- **Efficient Serving** - Optimized file reading and streaming
- **Caching Headers** - Includes appropriate cache-control headers

### Web Development
- **SPA Support** - Falls back to `index.html` for missing routes (perfect for React, Vue, Angular)
- **CORS Enabled** - All cross-origin requests allowed by default
- **Development Friendly** - Hot-reload compatible, detailed logging

### Security
- **Directory Traversal Protection** - Prevents access to files outside the specified directory
- **Path Canonicalization** - Resolves symbolic links and relative paths safely
- **Input Validation** - Validates all user inputs and parameters

### Cross-Platform
- **Native Binaries** - Compile to native executables for any platform
- **No Dependencies** - Single binary with no external runtime requirements
- **Build Scripts** - Included scripts for easy cross-compilation

## Common Use Cases

### Local Development
```bash
# Serve a React/Vue build
httpserver --directory ./dist

# Quick file sharing
httpserver --directory ./downloads --port 9000

# Test static sites
httpserver --directory ./public
```

### Production Deployment
```bash
# Serve static website
httpserver --directory /var/www/html --port 80

# Multiple sites on different ports
httpserver --directory /var/www/site1 --port 8001 &
httpserver --directory /var/www/site2 --port 8002 &
```

### Development Servers
```bash
# Documentation site
httpserver --directory ./docs --port 3000

# Component library
httpserver --directory ./storybook-static --port 6006
```

## Supported File Types

The server automatically detects and serves files with correct MIME types:

- **Web**: HTML, CSS, JavaScript, JSON, XML
- **Images**: PNG, JPEG, GIF, SVG, WebP, ICO
- **Documents**: PDF, TXT, Markdown
- **Fonts**: WOFF, WOFF2, TTF, OTF
- **Archives**: ZIP, TAR, GZ
- **And many more...**

## Building for Production

### Single Platform
```bash
cargo build --release
```

### Cross-Platform Build
Use the included build script:
```bash
# Unix/macOS
./build.sh

# Windows
build.bat
```

This creates optimized binaries for multiple platforms in the `dist/` directory.

## Performance

- **Memory Efficient** - Minimal memory footprint, suitable for resource-constrained environments
- **Fast Startup** - Near-instantaneous server startup
- **Concurrent Handling** - Built on Tokio for excellent concurrent request handling
- **Static Optimization** - Optimized specifically for static file serving

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

This project is open source and available under the MIT License.
