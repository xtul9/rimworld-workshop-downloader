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

## Installation

Download the latest release from [GitHub Releases](https://github.com/xtul9/rimworld-workshop-downloader/releases).

### Linux
- **Arch Linux & derivatives** (Manjaro, CachyOS, etc.): Install the `.pkg.tar.zst` package
  ```bash
  sudo pacman -U rimworld-workshop-downloader-*.pkg.tar.zst
  ```
- **Debian/Ubuntu**: Install the `.deb` package
  ```bash
  sudo dpkg -i rimworld-workshop-downloader_*.deb
  ```
- **Fedora/RHEL**: Install the `.rpm` package
  ```bash
  sudo rpm -i rimworld-workshop-downloader-*.rpm
  ```

### Windows
Run the `.msi` installer.

### macOS
Mount the `.dmg` file and drag the application to your Applications folder.

See the [Wiki](https://github.com/xtul9/rimworld-workshop-downloader/wiki) for more info.
