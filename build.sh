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

# Download SteamCMD for current platform
echo "Downloading SteamCMD for current platform..."
cd scripts
cargo build --release --bin download_steamcmd
./target/release/download_steamcmd
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

