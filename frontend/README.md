# Rimworld Workshop Downloader - Frontend

Frontend application built with Tauri, React, and TypeScript.

## Development

### Running in Development Mode

```bash
npm run tauri dev
```

Or use the helper script from the project root:

```bash
./run-dev.sh
```

### Building for Production

```bash
npm run build
npm run tauri build
```

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Project Structure

- `src/components/` - React components
- `src/contexts/` - React contexts for state management
- `src/utils/` - Utility functions
- `src-tauri/` - Tauri (Rust) source code
