#!/bin/bash

# Build script for macOS
echo "Building HTTP Server for macOS..."

# Parse command line arguments
BUILD_TYPE="debug"
BUILD_FLAG=""

if [[ "$1" == "--release" ]]; then
    BUILD_TYPE="release"
    BUILD_FLAG="--release"
fi

echo "Build type: $BUILD_TYPE"
echo ""

# Build for macOS
echo "Building for macOS ($BUILD_TYPE)..."
cargo build $BUILD_FLAG

if [ $? -eq 0 ]; then
    echo "macOS build successful"
    echo "Binary available at target/$BUILD_TYPE/httpserver"
    
    # Copy app_config.toml to build directory
    echo "Copying app_config.toml to build directory..."
    if [ -f "app_config.toml" ]; then
        cp app_config.toml target/$BUILD_TYPE/app_config.toml
        echo "Configuration file copied to target/$BUILD_TYPE/app_config.toml"
    else
        echo "Warning: app_config.toml not found, application will use defaults"
    fi
    
    echo ""
    echo "Usage:"
    echo "  ./target/$BUILD_TYPE/httpserver --help"
    echo "  ./target/$BUILD_TYPE/httpserver --directory ./public --port 8080"
    echo "  ./target/$BUILD_TYPE/httpserver --config config.simple.toml"
else
    echo "macOS build failed"
    exit 1
fi

echo "Build completed!"
