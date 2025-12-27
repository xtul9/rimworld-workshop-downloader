#!/bin/bash

# Script to run the project in development mode

echo "Starting Rimworld Mod Updater in development mode..."
echo ""

# Load Cargo (Rust) environment if it exists
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

# Check if npm is installed
if ! command -v npm &> /dev/null; then
    echo "Error: npm is not installed"
    exit 1
fi

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "Error: cargo (Rust) is not installed or not in PATH"
    echo "Try running: source \$HOME/.cargo/env"
    exit 1
fi

# Check if glib-2.0 is available
if ! pkg-config --exists glib-2.0 2>/dev/null; then
    echo "⚠️  WARNING: glib-2.0 is not installed!"
    echo "Install system dependencies for Tauri (see README.md)"
fi

# Check and install frontend dependencies if needed
if [ ! -d "frontend/node_modules" ]; then
    echo "Installing frontend dependencies..."
    cd frontend
    npm install
    cd ..
fi

# Start frontend dev server in background
echo "Starting frontend dev server..."
cd frontend
npm run dev > ../frontend.log 2>&1 &
FRONTEND_PID=$!
cd ..
echo "Frontend dev server PID: $FRONTEND_PID"
echo "Frontend logs are being written to: frontend.log"
echo ""

# Wait a moment for frontend to start
sleep 3

# Configure Wayland environment variables before starting Tauri
if [ "$XDG_SESSION_TYPE" = "wayland" ]; then
    # Remove x11 override if present
    [ "$GDK_BACKEND" = "x11" ] && unset GDK_BACKEND
    # Set Wayland backend and fix WebKit protocol error
    # See: https://github.com/tauri-apps/tauri/issues/10702
    export GDK_BACKEND=wayland
    export WEBKIT_DISABLE_DMABUF_RENDERER=1
fi

# Start Tauri application
echo "Starting Tauri application..."
cd backend
npx --prefix ../frontend tauri dev

# After completion, stop frontend
echo "Stopping frontend dev server..."
kill $FRONTEND_PID 2>/dev/null

