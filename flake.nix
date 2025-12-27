{
  description = "Rimworld Workshop Downloader - Desktop application for managing Rimworld mods from Steam Workshop";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Rust toolchain - using stable version
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rustfmt" "clippy" ];
        };

        # Node.js version
        nodejs = pkgs.nodejs_20;

        # System dependencies for Tauri
        tauriDeps = with pkgs; [
          webkitgtk
          openssl
          glib
          pkg-config
          curl
          wget
          file
          libxdo
          libappindicator
          librsvg
          wayland
          wayland-protocols
          libxkbcommon
          gtk3
          cairo
          gdk-pixbuf
          pango
          atk
          glib-networking
        ];

        # Build the application
        buildApp = pkgs.stdenv.mkDerivation {
          pname = "rimworld-workshop-downloader";
          version = "0.4.1";
          src = ./.;

          nativeBuildInputs = with pkgs; [
            rustToolchain
            nodejs
            pkg-config
            makeWrapper
          ] ++ tauriDeps;

          buildInputs = tauriDeps;

          # Set environment variables for Rust
          CARGO_HOME = ".cargo";
          RUST_BACKTRACE = "1";

          # Build phases
          buildPhase = ''
            # Download SteamCMD
            echo "Downloading SteamCMD..."
            cd scripts
            cargo build --release --bin download_steamcmd
            ./target/release/download_steamcmd
            cd ..

            # Build frontend
            echo "Building frontend..."
            cd frontend
            npm ci
            npm run build
            cd ..

            # Build Tauri application
            echo "Building Tauri application..."
            cd backend
            npx --prefix ../frontend tauri build
            cd ..
          '';

          installPhase = ''
            mkdir -p $out/share/rimworld-workshop-downloader

            # Copy built binaries and packages
            if [ -d backend/target/release/bundle ]; then
              cp -r backend/target/release/bundle/* $out/share/rimworld-workshop-downloader/ || true
            fi

            # Also copy the binary if it exists
            if [ -f backend/target/release/rimworld-workshop-downloader ]; then
              mkdir -p $out/bin
              cp backend/target/release/rimworld-workshop-downloader $out/bin/
              chmod +x $out/bin/rimworld-workshop-downloader
            fi

            # Create a helper script to install packages
            mkdir -p $out/bin
            cat > $out/bin/install-rimworld-downloader <<EOF
            #!/bin/sh
            if [ -f "$out/share/rimworld-workshop-downloader/deb/"*.deb ]; then
              echo "Installing .deb package..."
              sudo dpkg -i "$out/share/rimworld-workshop-downloader/deb/"*.deb
            elif [ -f "$out/share/rimworld-workshop-downloader/rpm/"*.rpm ]; then
              echo "Installing .rpm package..."
              sudo rpm -i "$out/share/rimworld-workshop-downloader/rpm/"*.rpm
            else
              echo "No package found. Binary is available at: $out/bin/rimworld-workshop-downloader"
            fi
            EOF
            chmod +x $out/bin/install-rimworld-downloader
          '';

          # Don't fail if some files don't exist
          dontFixup = true;
        };
      in
      {
        # Development shell
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            nodejs
            pkg-config
          ] ++ tauriDeps;

          shellHook = ''
            echo "Rimworld Workshop Downloader Development Environment"
            echo "=================================================="
            echo ""
            echo "Available commands:"
            echo "  npm run tauri dev    - Run in development mode"
            echo "  ./run-dev.sh         - Run in development mode (but more, uh, automated i guess)"
            echo "  ./build.sh           - Build for production"
            echo ""
            echo "Rust version: $(rustc --version)"
            echo "Node version: $(node --version)"
            echo "Cargo version: $(cargo --version)"
            echo ""
            
            # Set environment variables for Tauri
            export WEBKIT_DISABLE_DMABUF_RENDERER=1
            export CARGO_HOME="$PWD/.cargo"
            export RUST_BACKTRACE=1
          '';
        };

        # Default package (builds the application)
        packages.default = buildApp;

        # Formatter (optional, for nix fmt)
        formatter = pkgs.nixpkgs-fmt;
      }
    );
}

