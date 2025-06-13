#!/bin/bash

# Clean script for macOS
echo "Cleaning Rust build artifacts on macOS..."

# Remove target directory and all its contents
if [ -d "target" ]; then
    echo "Removing target directory..."
    rm -rf target
    echo "Target directory removed"
else
    echo "Target directory does not exist"
fi

echo "Cleanup completed!"
