#!/usr/bin/env bash
# Mugen Installer — zero-terminal for end users.
# Double-click "Install Mugen.desktop" or run: bash install.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${CYAN}[mugen]${NC} $*"; }
ok()    { echo -e "${GREEN}[mugen]${NC} $*"; }
err()   { echo -e "${RED}[mugen]${NC} $*" >&2; }

# --- Pre-flight checks ---
if [[ "$(uname)" != "Linux" ]]; then
    err "Mugen requires Linux (SteamOS). Detected: $(uname)"
    exit 1
fi

if [[ ! -f "$SCRIPT_DIR/mugen-daemon" ]]; then
    err "mugen-daemon not found in $SCRIPT_DIR"
    exit 1
fi

if [[ ! -d "$SCRIPT_DIR/ui" ]]; then
    err "ui/ folder not found in $SCRIPT_DIR"
    exit 1
fi

echo ""
info "${BOLD}Mugen Installer${NC}"
echo ""

# --- Create directories ---
info "Setting up directories..."
mkdir -p ~/.local/bin
mkdir -p ~/.config/mugen/apps
mkdir -p ~/.config/systemd/user
mkdir -p ~/.local/share/mugen/{launcher,apps,profiles,logs,cache}
mkdir -p ~/.local/share/mugen/launcher/ui
mkdir -p ~/.local/share/applications

# --- Stop existing daemon if running ---
systemctl --user stop mugen.service 2>/dev/null || true

# --- Copy daemon ---
info "Installing daemon..."
cp "$SCRIPT_DIR/mugen-daemon" ~/.local/bin/mugen-daemon
chmod +x ~/.local/bin/mugen-daemon
ok "Daemon installed"

# --- Copy frontend files ---
info "Installing launcher UI..."
cp -r "$SCRIPT_DIR/ui/"* ~/.local/share/mugen/launcher/ui/
ok "Launcher UI installed"

# --- Create launcher script (opens Firefox in kiosk mode) ---
cat > ~/.local/bin/mugen-launcher << 'EOF'
#!/bin/bash
# Mugen Launcher — opens the UI in Firefox kiosk mode
exec flatpak run org.mozilla.firefox --kiosk http://127.0.0.1:7331/ui/
EOF
chmod +x ~/.local/bin/mugen-launcher

# --- Create .desktop entry ---
cat > ~/.local/share/applications/mugen.desktop << EOF
[Desktop Entry]
Name=Mugen
Comment=Mugen Launcher
Exec=$HOME/.local/bin/mugen-launcher
Type=Application
Categories=Game;
Terminal=false
EOF

# --- Install systemd service ---
info "Setting up auto-start..."
cat > ~/.config/systemd/user/mugen.service << 'EOF'
[Unit]
Description=Mugen Daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=%h/.local/bin/mugen-daemon
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=default.target
EOF

systemctl --user daemon-reload
systemctl --user enable mugen.service
systemctl --user start mugen.service
ok "Daemon running and set to auto-start on boot"

# --- Enable lingering (survive logout/reboot) ---
if command -v loginctl &>/dev/null; then
    loginctl enable-linger "$(whoami)" 2>/dev/null || true
fi

# --- Verify ---
echo ""
echo "============================================"
echo ""

sleep 2
if curl -sf http://127.0.0.1:7331/health >/dev/null 2>&1; then
    ok "${BOLD}Mugen installed successfully!${NC}"
    echo ""
    ok "The daemon is running in the background."
    ok "It will auto-start every time you turn on your Deck."
    echo ""
    info "To add Mugen to Steam:"
    info "  1. Add a Non-Steam Game"
    info "  2. Press Ctrl+H to show hidden files"
    info "  3. Browse to: /home/deck/.local/bin/mugen-launcher"
    info "  4. Add it, then launch from your library"
    echo ""
    info "Or test now by opening Firefox and going to:"
    info "  http://127.0.0.1:7331/ui/"
else
    err "Something went wrong. The daemon didn't start."
    err "Try rebooting your Deck and running this installer again."
fi

echo ""
