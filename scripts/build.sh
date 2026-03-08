#!/usr/bin/env bash
# Production build script for all Mugen components
set -eu

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

echo "[mugen] Building all components..."

# Daemon
echo "[mugen] Building daemon..."
(cd "$ROOT_DIR/daemon" && cargo build --release)
echo "[mugen] Daemon built: daemon/target/release/mugen-daemon"

# Launcher (React SPA — served by daemon)
echo "[mugen] Building launcher..."
(cd "$ROOT_DIR/launcher" && npm run build)
echo "[mugen] Launcher built: launcher/dist/"

# Backend
echo "[mugen] Building backend..."
(cd "$ROOT_DIR/backend" && npm run build)
echo "[mugen] Backend built: backend/dist/"

# Package for deployment
echo "[mugen] Packaging for deployment..."
INSTALL_DIR="$ROOT_DIR/mugen-install"
rm -rf "$INSTALL_DIR"
mkdir -p "$INSTALL_DIR/ui"
cp "$ROOT_DIR/daemon/target/release/mugen-daemon" "$INSTALL_DIR/"
cp -r "$ROOT_DIR/launcher/dist/"* "$INSTALL_DIR/ui/"
cp "$ROOT_DIR/installer/install.sh" "$INSTALL_DIR/"

echo "[mugen] Package ready: mugen-install/"
echo "[mugen] All builds complete."
