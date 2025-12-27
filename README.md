# Rimworld Workshop Downloader

Desktop application for managing Rimworld mods from Steam Workshop. Built with Tauri on the backend, React on the frontend.

## Features

- **Query & Update Mods**: Check for outdated mods in your Rimworld mods folder and update them automatically
- **Download Mods**: Download mods directly from Steam Workshop by ID or URL
- **Installed Mods Management**: View all installed mods with search, sort, and update capabilities
- **Parallel Downloads**: 
  - Automatic parallel downloading using up to 4 SteamCMD instances
  - Size-based load balancing for optimal performance
  - Real-time progress updates via file system watching
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
├── frontend/                        # React frontend
│   ├── components/                  # React components
│   │   ├── QueryTab.tsx             # Query & Update tab
│   │   ├── DownloadTab.tsx          # Download mods tab
│   │   ├── SettingsTab.tsx          # Settings tab
│   │   ├── ModList.tsx              # Virtualized mod list
│   │   ├── ContextMenu.tsx          # Global context menu
│   │   └── RestoreBackupModal.tsx   # Backup restore modal
│   ├── contexts/                    # React contexts
│   │   ├── SettingsContext.tsx      # Application settings
│   │   ├── ModsContext.tsx          # Mods state management
│   │   ├── ModsPathContext.tsx      # Mods path management
│   │   ├── ModalContext.tsx         # Global modal management
│   │   └── ContextMenuContext.tsx   # Global context menu
│   ├── utils/                       # Utilities
│   │   ├── settingsStorage.ts       # Settings persistence
│   │   └── api.ts                   # API client (Tauri invoke)
│   ├── main.tsx                     # React entry point
│   └── package.json                 # Frontend dependencies
├── backend/                         # Rust (Tauri) backend
│   ├── src/
│   │   ├── main.rs                  # Tauri entry point
│   │   ├── lib.rs                   # Tauri library
│   │   ├── commands.rs              # Tauri commands (API)
│   │   └── backend/                 # Rust backend modules
│   │       ├── mod_query.rs         # Query mods for updates
│   │       ├── mod_updater.rs       # Update mods logic
│   │       ├── downloader.rs        # Download mods via SteamCMD
│   │       ├── steam_api.rs         # Steam API client
│   │       ├── cache.rs             # API response caching
│   │       └── rate_limiter.rs      # Rate limiting for API calls
│   ├── Cargo.toml                   # Rust dependencies
│   └── tauri.conf.json              # Tauri configuration
├── scripts/                         # Build scripts
│   └── src/
│       └── main.rs                  # SteamCMD downloader (Rust)
└── bin/                             # Binary dependencies
    └── steamcmd/                    # SteamCMD binaries
```

## Requirements

### Development
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

2. Install frontend dependencies:
```bash
cd ../frontend
npm install
```

## Running

### Development Mode

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

1. Download SteamCMD:
```bash
cd scripts
cargo build --release --bin download_steamcmd
./target/release/download_steamcmd
cd ..
```

2. Build the Tauri application:
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
- **SteamCMD is automatically downloaded and bundled** during the build process - users don't need to install it
- The Rust backend is compiled directly into the Tauri application
- **No separate backend process** - everything runs in a single application

## Architecture

### Communication Between Components

- **Frontend (React)** communicates with **Backend (Rust)** via Tauri commands (`invoke()`)
- **Real-time Updates**: Backend emits Tauri events for download/update progress
- All backend logic runs in the same process as the Tauri application
- No HTTP server or separate process required

### Performance Optimizations

- **Parallel Mod Downloads**: Uses up to 4 parallel SteamCMD instances for faster downloads
  - Size-based load balancing distributes mods by file size across instances
  - Automatic instance count calculation based on number of mods
- **File System Watching**: Uses native file system events (inotify/kqueue/ReadDirectoryChangesW) instead of polling
  - Instant detection when mods are downloaded
  - Lower CPU usage compared to polling
- **Parallel API Queries**: Batch queries to Steam API with parallel processing
- **Parallel Mod Updates**: Multiple mods can be updated simultaneously after download

### Tauri Commands (API)

The application uses Tauri commands instead of HTTP endpoints:

- `query_mods(modsPath, ignoredMods)` - Query mods folder for outdated mods
- `list_installed_mods(modsPath)` - List all installed mods (fast, local data only)
- `update_mod_details(mods)` - Fetch detailed Steam API information for mods
- `update_mods(mods, backupDirectory)` - Update selected mods (with optional backup)
  - Uses parallel SteamCMD instances for faster downloads
  - Emits real-time events: `mod-downloaded`, `mod-updated`
- `check_backup(modPath, backupDirectory)` - Check if backup exists for a mod
- `check_backups(mods, backupDirectory)` - Batch check backups for multiple mods
- `restore_backup(modPath, backupDirectory)` - Restore mod from backup
- `restore_backups(mods, backupDirectory)` - Batch restore backups for multiple mods
- `ignore_update(mods)` - Ignore specific update (creates .ignoredupdate file)
- `undo_ignore_update(mods)` - Undo ignored update
- `check_ignored_updates(mods)` - Check which mods have ignored updates
- `get_file_details(modId)` - Get mod details from Steam Workshop
- `get_file_details_batch(modIds)` - Batch get mod details
- `is_collection(modId)` - Check if file is a collection
- `is_collection_batch(modIds)` - Batch check if files are collections
- `get_collection_details(collectionId)` - Get collection details
- `get_collection_details_batch(collectionIds)` - Batch get collection details
- `download_mod(modId, modsPath)` - Download mod from Steam Workshop

### Tauri Events

The application emits real-time events for download/update progress:

- `mod-downloaded` - Emitted when a mod is downloaded by SteamCMD
  - Payload: `{ modId: string }`
- `mod-updated` - Emitted when a mod is successfully updated or fails
  - Payload: `{ modId: string, success: boolean, error?: string }`

## Development

### Adding New Tauri Commands

1. Add a new function in `backend/src/commands.rs` with `#[tauri::command]` attribute
2. Register the command in `backend/src/lib.rs` in the `invoke_handler`
3. Call the command from the frontend using `invoke()` from `@tauri-apps/api/core`

### Adding New React Features

1. Edit components in `frontend/components/`
2. Use React contexts in `frontend/contexts/` for global state management
3. Use `invoke()` from `@tauri-apps/api/core` to call Rust backend commands
4. For Tauri-specific features, use `@tauri-apps/api` or Tauri plugins

### Key Technologies

- **Frontend**: React 19, TypeScript, Tauri 2
- **Backend**: Rust (compiled into Tauri application)
- **Steam Integration**: SteamCMD for downloading mods
- **State Management**: React Context API
- **UI**: Custom CSS with dark/light mode support
- **List Virtualization**: react-window for performance
- **File System Watching**: notify crate for efficient file system event monitoring
- **Parallel Processing**: tokio for async/await and parallel task execution

## CI/CD and Releases

### Automated Builds

The project includes GitHub Actions workflows that automatically build the application for all platforms (Linux, Windows, macOS) when:

- Code is pushed to the `master` branch
- A tag starting with `v` is created (e.g., `v0.1.0`)
- The workflow is manually triggered from the Actions tab

### Creating a Release

To create a new release:

1. **Update the version** in the following files:
   - `backend/tauri.conf.json`
   - `backend/Cargo.toml`
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
- Make sure Rust dependencies are installed correctly
- Check if SteamCMD is available in `bin/steamcmd/`

### Cargo Not Found
Run: `source $HOME/.cargo/env` or add it to your `~/.bashrc` / `~/.zshrc`

## Notes

- **Rust backend is compiled directly into the application** - no separate process required
- **SteamCMD is embedded in the application** - automatically downloaded during build and bundled with the app
- Settings are persisted using Tauri's plugin-store
- First run experience: Settings tab opens by default on first launch

## License

This project is licensed under the MIT License.

### Embedded Components

This application includes the following third-party components:

#### SteamCMD

The application bundles **SteamCMD** command-line tool for downloading Steam Workshop content.

- **License**: Steam Subscriber Agreement (proprietary, owned by Valve Corporation)
- **Source**: https://developer.valvesoftware.com/wiki/SteamCMD
- **Important Notes**:
  - SteamCMD is **not open-source**
  - SteamCMD is licensed separately by Valve Corporation
  - The SteamCMD binary is **not modified** in any way
  - Users must accept Valve's Steam Subscriber Agreement when using SteamCMD
  - This application does **not** sublicense SteamCMD under MIT
  - SteamCMD is included for user convenience - users do not need to install it separately

**Legal Notice**: This software bundles SteamCMD, which is licensed separately by Valve Corporation under the Steam Subscriber Agreement. By using this application, you acknowledge that SteamCMD is subject to Valve's terms and conditions, not this project's MIT license.

### License Summary

- **This Project**: MIT License
- **SteamCMD**: Steam Subscriber Agreement (separate license, owned by Valve)

For the full license text, see the [LICENSE](LICENSE) file in the repository root.
