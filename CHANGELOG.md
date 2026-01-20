# Changelog

## v0.6.1

### Added
- A dropdown menu at the top right corner of RWD window
- An option within aforementioned menu to view app's version
- Ability to export the mod list to clipboard. The output is compatible with RimSort's output and thus can be used with tools such as Judge My List (https://jumli.sysrqmagician.dev/)

## v0.6.0

### Added
- Ability to cancel mod updates and downloads at any time using cancel buttons in the interface
- The app now detects corrupted mods and warns you before installation
- When a corrupted mod is detected, you can choose to overwrite it or install it with a different name
- Cancel buttons are now available in all tabs (Query, Installed Mods, and Download)
- Arch Linux users can now install the app using the `.pkg.tar.zst` package (via AUR too, probably soon)

### Changed
- The app now uses system's SteamCMD installation when available, which is better on systems such as Arch or NixOS
- Improved handling of mod name conflicts - the app generates better unique names automatically

### Fixed
- Fixed SteamCMD console window opening on Windows
- Some improvements to real-time updates in UI as mods are being updated or downloaded

## v0.5.1

### Added
- Automatic mod list updates when mods are added or removed from the directory
- Better error handling when the app doesn't have access to the mod directory

### Changed
- Improved mod list sorting consistency when mods are added or removed while the app is running
- Improved context menu behavior when right-clicking a mod with nothing selected
- Better handling of app access restrictions based on file permissions

### Fixed
- Fixed mod list not updating correctly in some cases
- Fixed issues with mod updates failing
- Fixed problems with restoring backups of deleted mods
- Fixed various state synchronization issues
- Fixed issues with symbolic links in mod directories
