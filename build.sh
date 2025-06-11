#!/bin/bash

# Build script for cross-platform HTTP server
echo "🚀 Building HTTP Server for multiple platforms..."

# Function to build for a specific target
build_target() {
    local target=$1
    local output_name=$2
    
    echo "Building for $target..."
    
    if cargo build --release --target "$target"; then
        echo "Successfully built for $target"
        
        # Create platform-specific directory
        mkdir -p "dist/$target"
        
        # Copy binary with appropriate extension
        if [[ $target == *"windows"* ]]; then
            cp "target/$target/release/httpserver.exe" "dist/$target/httpserver-$output_name.exe"
        else
            cp "target/$target/release/httpserver" "dist/$target/httpserver-$output_name"
        fi
        
        echo "📁 Binary saved to dist/$target/httpserver-$output_name"
    else
        echo "Failed to build for $target"
    fi
    echo ""
}

# Create dist directory
mkdir -p dist

# Build for current platform first
echo "🔨 Building for current platform (native)..."
cargo build --release
if [ $? -eq 0 ]; then
    echo "Native build successful"
    cp target/release/httpserver dist/httpserver-native
    echo "📁 Native binary saved to dist/httpserver-native"
else
    echo "Native build failed"
    exit 1
fi
echo ""

# Check if cross-compilation tools are available
echo "Checking for cross-compilation targets..."

# Install common targets if not already installed
echo "Installing cross-compilation targets..."
rustup target add x86_64-unknown-linux-gnu 2>/dev/null || echo "Linux target already installed or unavailable"
rustup target add x86_64-pc-windows-gnu 2>/dev/null || echo "Windows target already installed or unavailable"
rustup target add x86_64-apple-darwin 2>/dev/null || echo "macOS Intel target already installed or unavailable"
rustup target add aarch64-apple-darwin 2>/dev/null || echo "macOS ARM target already installed or unavailable"

echo ""
echo "Available targets for cross-compilation:"
rustup target list --installed | grep -E "(linux|windows|darwin)"
echo ""

# Attempt cross-compilation for common platforms
echo "Attempting cross-compilation..."

# Linux (if not already on Linux)
if [[ "$OSTYPE" != "linux-gnu"* ]]; then
    build_target "x86_64-unknown-linux-gnu" "linux-x64"
fi

# Windows (if not already on Windows)
if [[ "$OSTYPE" != "msys" ]] && [[ "$OSTYPE" != "cygwin" ]]; then
    build_target "x86_64-pc-windows-gnu" "windows-x64"
fi

# macOS targets (if on macOS)
if [[ "$OSTYPE" == "darwin"* ]]; then
    # Build for Intel Macs
    build_target "x86_64-apple-darwin" "macos-intel"
    
    # Build for Apple Silicon Macs
    build_target "aarch64-apple-darwin" "macos-arm64"
fi

echo "Build process completed!"
echo ""
echo "Summary:"
echo "  Native binary: dist/httpserver-native"
ls -la dist/ 2>/dev/null || echo "  No additional cross-compiled binaries created"
echo ""
echo "Usage:"
echo "  ./dist/httpserver-native --help"
echo "  ./dist/httpserver-native --directory ./public --port 8080"
