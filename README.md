# Rimworld Mod Updater

Desktop application using Tauri (React) as frontend and Node.js (Express) as backend API.

**Native Wayland support** - the project uses modern solutions and fully supports Wayland on Linux.

## Project Structure

```
rimworld-mod-updater-multiplatform/
├── backend/                         # Node.js backend (Express + TypeScript)
│   ├── src/
│   │   ├── index.ts                # Main server file
│   │   └── routes/
│   │       └── mod.ts              # API routes
│   └── package.json
└── frontend/                        # Tauri + React frontend
    ├── src/                         # React source code
    └── src-tauri/                   # Rust (Tauri) source code
```

## Requirements

### Backend (Node.js)
- Node.js (version 18 or newer)
- npm

### Frontend (Tauri)
- Node.js (version 18 or newer)
- npm
- Rust and Cargo (latest stable version)
  - Install using: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
  - After installation you may need to run: `source $HOME/.cargo/env`
- System dependencies for Tauri

### Installing System Dependencies

#### Fedora / RHEL / CentOS
```bash
sudo dnf install -y \
    glib2-devel \
    webkit2gtk4.1-devel \
    openssl-devel \
    curl \
    wget \
    file \
    libxdo-devel \
    libappindicator-gtk3-devel \
    librsvg2-devel \
    pkg-config \
    wayland-devel \
    wayland-protocols-devel \
    libxkbcommon-devel
```

#### Debian / Ubuntu
```bash
sudo apt update
sudo apt install -y \
    libwebkit2gtk-4.1-dev \
    build-essential \
    curl \
    wget \
    file \
    libxdo-dev \
    libssl-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    pkg-config \
    libwayland-dev \
    wayland-protocols \
    libxkbcommon-dev
```

#### Arch Linux
```bash
sudo pacman -S \
    webkit2gtk \
    base-devel \
    curl \
    wget \
    openssl \
    libxdo \
    libappindicator \
    librsvg \
    pkg-config \
    wayland \
    wayland-protocols \
    libxkbcommon
```

## Installation

1. Clone the repository:
```bash
git clone <repo-url>
cd rimworld-mod-updater-multiplatform
```

2. Install backend dependencies:
```bash
cd backend
npm install
```

3. Install frontend dependencies:
```bash
cd ../frontend
npm install
```

## Running

### Development Mode

The Node.js backend will start automatically when launching the Tauri application. If you want to start the backend manually:

```bash
cd backend
npm run dev
```

The backend will be available at: `http://localhost:5000`

To run the Tauri application:

```bash
cd frontend
npm run tauri dev
```

Or use the helper script:

```bash
./run-dev.sh
```

### Production Build

To build a single executable/bundle:

**Option 1: Use the build script (recommended)**
```bash
./build.sh
```

**Option 2: Manual build**

1. Build the Node.js backend:
```bash
cd backend
npm install  # Install all dependencies including devDependencies for TypeScript
npm run build  # Compile TypeScript to JavaScript
npm install --omit=dev  # Install only production dependencies for bundling
```

2. Build the Tauri application (this will bundle the backend):
```bash
cd ../frontend
npm run build
npm run tauri build
```

The built application will be in `frontend/src-tauri/target/release/bundle/`:
- **Linux**: `.AppImage`, `.deb`, or `.rpm` (depending on your system)
- **Windows**: `.msi` or `.exe`
- **macOS**: `.dmg` or `.app`

**Important Notes**:
- The backend Node.js files are bundled with the application as resources
- **Node.js runtime must be installed** on the target system (the application uses the system's Node.js)
- The application will automatically start the bundled backend when launched
- For a truly standalone executable (without requiring Node.js), you would need to bundle Node.js runtime or use a different approach (e.g., pkg, nexe, or compile Node.js backend to native code)

## Architecture

### Communication Between Components

- **Frontend (React)** communicates with **Backend (Node.js)** via HTTP REST API on port 5000
- **Tauri (Rust)** starts the Node.js backend process when the application launches
- The Node.js backend runs as a separate process and can also be run independently

### API Endpoints

- `GET /api/mod/greet?name={name}` - Example greeting endpoint
- `GET /api/mod/status` - Backend status
- `GET /api/health` - Health check endpoint

## Development

### Adding New API Endpoints

1. Add new routes in `backend/src/routes/`
2. Import and use the router in `backend/src/index.ts`
3. The endpoint will be automatically available at `/api/{route}/{endpoint}`

### Adding New React Features

1. Edit components in `frontend/src/`
2. Use `fetch()` to communicate with the Node.js backend at `http://localhost:5000`

## Troubleshooting

### Rust Compilation Error: Missing glib-2.0
Install system dependencies according to the instructions above for your Linux distribution.

### Wayland Error: Error 71 (Protocol error)
The project natively supports Wayland. If you encounter a Wayland protocol error:
1. Make sure you have installed all Wayland dependencies (see installation section above)
2. Check if you're using a Wayland session: `echo $XDG_SESSION_TYPE` (should return `wayland`)
3. The `run-dev.sh` script automatically sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` for Wayland sessions, which fixes a known WebKit issue (see [issue #10702](https://github.com/tauri-apps/tauri/issues/10702))
4. If running the application manually, use: `WEBKIT_DISABLE_DMABUF_RENDERER=1 npm run tauri dev`
5. If the problem persists, check the application logs in the terminal

### Window Closes Immediately After Opening
- Check logs in the terminal - they may contain error information
- Make sure the Node.js backend starts correctly
- Check if port 5000 is not occupied by another application

### Cargo Not Found
Run: `source $HOME/.cargo/env` or add it to your `~/.bashrc` / `~/.zshrc`

### Backend Doesn't Start Automatically
Run the backend manually in a separate terminal: `cd backend && npm run dev`

## Notes

- The Node.js backend must be running on port 5000
- CORS is configured to allow all origins (this should be changed in production)
- The backend automatically starts when the Tauri application launches
- The backend uses TypeScript and is compiled to JavaScript before running

## License

[License placeholder]
