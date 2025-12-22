#!/bin/bash

# Script to build the application for production

set -e

echo "Building Rimworld Mod Updater for production..."
echo ""

# Load Cargo (Rust) environment if it exists
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "Error: cargo (Rust) is not installed or not in PATH"
    echo "Try running: source \$HOME/.cargo/env"
    exit 1
fi

# Build backend
echo "Building Node.js backend..."
cd backend
# Install all dependencies (including devDependencies for TypeScript and pkg)
npm install
# Build TypeScript to JavaScript
npm run build

# Download SteamCMD for current platform
echo "Downloading SteamCMD for current platform..."
npm run build:steamcmd

# Build sidecar binaries (only for current platform to save time)
echo "Building sidecar binary for current platform (this may take a while)..."
echo "Note: For other platforms, build on those platforms or use CI/CD"
npm run build:sidecar
cd ..

# Build frontend
echo "Building Tauri frontend..."
cd frontend
npm run build

# Build Tauri application (bundles backend automatically)
echo "Bundling Tauri application..."
npm run tauri build

echo ""
echo "Build complete! Output files are in: frontend/src-tauri/target/release/bundle/"
echo ""
echo "Built packages:"
echo "  - .deb: frontend/src-tauri/target/release/bundle/deb/"
echo "  - .rpm: frontend/src-tauri/target/release/bundle/rpm/"
echo ""
echo "Note: AppImage build was skipped due to linuxdeploy issues."
echo "You can install the .deb or .rpm package instead."

