#!/bin/bash

# Build script for Linux
echo "Building HTTP Server for Linux..."

# Parse command line arguments
BUILD_TYPE="debug"
BUILD_FLAG=""

if [[ "$1" == "--release" ]]; then
    BUILD_TYPE="release"
    BUILD_FLAG="--release"
fi

echo "Build type: $BUILD_TYPE"
echo ""

# Build for Linux
echo "Building for Linux ($BUILD_TYPE)..."
cargo build $BUILD_FLAG

if [ $? -eq 0 ]; then
    echo "Linux build successful"
    echo "Binary available at target/$BUILD_TYPE/httpserver"
    echo ""
    echo "Usage:"
    echo "  ./target/$BUILD_TYPE/httpserver --help"
    echo "  ./target/$BUILD_TYPE/httpserver --directory ./public --port 8080"
else
    echo "Linux build failed"
    exit 1
fi

echo "Build completed!"
