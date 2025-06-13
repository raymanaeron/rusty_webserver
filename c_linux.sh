#!/bin/bash

# Clean script for Linux
echo "Cleaning Rust build artifacts on Linux..."

# Remove target directory and all its contents
if [ -d "target" ]; then
    echo "Removing target directory..."
    rm -rf target
    echo "Target directory removed"
else
    echo "Target directory does not exist"
fi

echo "Cleanup completed!"
