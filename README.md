# Rimworld Workshop Downloader

Desktop application for managing Rimworld mods from Steam Workshop. Built with Tauri (React) as frontend and Node.js (Express) as backend API.

**Native Wayland support** - the project uses modern solutions and fully supports Wayland on Linux.

## Features

- **Query & Update Mods**: Check for outdated mods in your Rimworld mods folder and update them automatically
- **Download Mods**: Download mods directly from Steam Workshop by ID or URL
- **Mod Management**: 
  - Ignore mods in three ways: temporarily from list, ignore specific update, or permanently ignore
  - Manage ignored mods list in Settings
- **Backup System**: 
  - Automatic backups before updating mods (configurable)
  - Restore mods from backups
  - Dedicated backup directory support
- **Dark/Light Mode**: System theme support with manual override
- **Virtualized Mod List**: Efficient rendering of large mod lists
- **Smart Selection**: Shift-click range selection and improved single-click behavior

## Project Structure

```
rimworld-mod-updater-multiplatform/
├── backend/                         # Node.js backend (Express + TypeScript)
│   ├── src/
│   │   ├── index.ts                # Main server file
│   │   ├── routes/
│   │   │   ├── mod.ts              # Mod management API routes
│   │   │   └── workshop.ts         # Steam Workshop API routes
│   │   └── services/
│   │       ├── modQuery.ts         # Query mods for updates
│   │       ├── modUpdater.ts       # Update mods logic
│   │       ├── downloader.ts       # Download mods via SteamCMD
│   │       ├── cache.ts            # API response caching
│   │       └── rateLimiter.ts      # Rate limiting for API calls
│   └── package.json
└── frontend/                        # Tauri + React frontend
    ├── src/
    │   ├── components/             # React components
    │   │   ├── QueryTab.tsx        # Query & Update tab
    │   │   ├── DownloadTab.tsx    # Download mods tab
    │   │   ├── SettingsTab.tsx     # Settings tab
    │   │   ├── ModList.tsx         # Virtualized mod list
    │   │   ├── ContextMenu.tsx     # Global context menu
    │   │   └── RestoreBackupModal.tsx # Backup restore modal
    │   ├── contexts/               # React contexts
    │   │   ├── SettingsContext.tsx # Application settings
    │   │   ├── ModsContext.tsx     # Mods state management
    │   │   ├── ModsPathContext.tsx # Mods path management
    │   │   ├── ModalContext.tsx    # Global modal management
    │   │   └── ContextMenuContext.tsx # Global context menu
    │   └── utils/                  # Utilities
    │       ├── settingsStorage.ts   # Settings persistence
    │       └── api.ts              # API client
    └── src-tauri/                   # Rust (Tauri) source code
        ├── src/
        │   ├── main.rs              # Tauri entry point
        │   └── lib.rs               # Tauri library
        └── tauri.conf.json          # Tauri configuration
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
- The backend Node.js application is compiled to a native executable using `pkg` and bundled as a sidecar
- **Node.js runtime is NOT required** on the target system - it's embedded in the application
- **SteamCMD is automatically downloaded and bundled** during the build process - users don't need to install it
- The application will automatically start the bundled backend when launched
- The sidecar binary contains the entire Node.js runtime and all dependencies

## Architecture

### Communication Between Components

- **Frontend (React)** communicates with **Backend (Node.js)** via HTTP REST API on port 5000
- **Tauri (Rust)** starts the Node.js backend process when the application launches
- The Node.js backend runs as a separate process and can also be run independently

### API Endpoints

#### Mod Management (`/api/mod`)
- `GET /api/mod/query?modsPath={path}&ignoredMods={ids}` - Query mods folder for outdated mods
- `POST /api/mod/update` - Update selected mods (with optional backup)
- `GET /api/mod/check-backup?modPath={path}&backupDirectory={dir}` - Check if backup exists for a mod
- `POST /api/mod/restore-backup` - Restore mod from backup
- `POST /api/mod/ignore-update` - Ignore specific update (creates .lastupdated file)
- `GET /api/mod/status` - Backend status
- `GET /api/mod/greet?name={name}` - Example greeting endpoint

#### Steam Workshop (`/api/workshop`)
- `GET /api/workshop/file-details?id={modId}` - Get mod details from Steam Workshop
- `GET /api/workshop/is-collection?id={modId}` - Check if file is a collection
- `GET /api/workshop/collection-details?id={collectionId}` - Get collection details
- `POST /api/workshop/download` - Download mod(s) from Steam Workshop

#### Health Check
- `GET /api/health` - Health check endpoint

## Development

### Adding New API Endpoints

1. Add new routes in `backend/src/routes/` (create new router or extend existing)
2. Import and use the router in `backend/src/index.ts`
3. The endpoint will be automatically available at `/api/{route}/{endpoint}`

### Adding New React Features

1. Edit components in `frontend/src/components/`
2. Use React contexts in `frontend/src/contexts/` for global state management
3. Use `fetch()` to communicate with the Node.js backend at `http://localhost:5000`
4. For Tauri-specific features, use `@tauri-apps/api` or Tauri plugins

### Key Technologies

- **Frontend**: React 19, TypeScript, Tauri 2
- **Backend**: Node.js, Express, TypeScript
- **Steam Integration**: SteamCMD for downloading mods
- **State Management**: React Context API
- **UI**: Custom CSS with dark/light mode support
- **List Virtualization**: react-window for performance

## CI/CD and Releases

### Automated Builds

The project includes GitHub Actions workflows that automatically build the application for all platforms (Linux, Windows, macOS) when:

- Code is pushed to the `master` branch
- A tag starting with `v` is created (e.g., `v0.1.0`)
- The workflow is manually triggered from the Actions tab

### Creating a Release

To create a new release:

1. **Update the version** in the following files:
   - `frontend/src-tauri/tauri.conf.json`
   - `frontend/src-tauri/Cargo.toml`
   - `frontend/package.json`

   Or use the provided script:
   ```bash
   ./bump-version.sh 0.1.0
   ```

2. **Commit and push** the changes:
   ```bash
   git add .
   git commit -m "Bump version to 0.1.0"
   git push origin master
   ```

3. **Create and push a tag**:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

4. **GitHub Actions will automatically**:
   - Build the application for Linux, Windows, and macOS
   - Create a GitHub Release with all binaries attached
   - The release will be available at: `https://github.com/YOUR_USERNAME/rimworld-mod-updater-multiplatform/releases`

### Build Artifacts

The workflow produces the following packages:

- **Linux**: `.deb` (Debian/Ubuntu) and `.rpm` (Fedora/RHEL) packages
- **Windows**: `.msi` installer
- **macOS**: `.dmg` disk image

All artifacts are automatically attached to the GitHub Release.

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

- The Node.js backend must be running on port 5000 (starts automatically)
- CORS is configured to allow all origins (this should be changed in production)
- The backend automatically starts when the Tauri application launches
- The backend uses TypeScript and is compiled to JavaScript, then bundled into a native executable using `pkg`
- **Node.js is embedded in the application** - users don't need to install it separately
- **SteamCMD is embedded in the application** - automatically downloaded during build and bundled with the app
- Settings are persisted using Tauri's plugin-store
- First run experience: Settings tab opens by default on first launch

## License

[License placeholder]
