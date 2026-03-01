#!/usr/bin/env bash
# Production build script for all Mugen components
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

echo "[mugen] Building all components..."

# Daemon
echo "[mugen] Building daemon..."
(cd "$ROOT_DIR/daemon" && cargo build --release)
echo "[mugen] Daemon built: daemon/target/release/mugen-daemon"

# Launcher
echo "[mugen] Building launcher..."
(cd "$ROOT_DIR/launcher" && npm run tauri build)
echo "[mugen] Launcher built"

# Backend
echo "[mugen] Building backend..."
(cd "$ROOT_DIR/backend" && npm run build)
echo "[mugen] Backend built: backend/dist/"

# SharkDeck
echo "[mugen] Building SharkDeck..."
(cd "$ROOT_DIR/apps/sharkdeck" && npm run build)
echo "[mugen] SharkDeck built: apps/sharkdeck/dist/"

echo "[mugen] All builds complete."
