#!/usr/bin/env bash
# Start all Mugen services for development
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

trap 'kill $(jobs -p) 2>/dev/null; exit' INT TERM

echo "[mugen] Starting development services..."

# Daemon
echo "[mugen] Starting daemon..."
(cd "$ROOT_DIR/daemon" && cargo run) &

# Backend
echo "[mugen] Starting backend..."
(cd "$ROOT_DIR/backend" && npm run dev) &

# Launcher (Tauri dev)
echo "[mugen] Starting launcher..."
(cd "$ROOT_DIR/launcher" && npm run tauri dev) &

echo "[mugen] All services started. Press Ctrl+C to stop."
wait
