#!/usr/bin/env bash
# Mugen Installer — curl -L mugen.gg/install | bash
# Installs Mugen Daemon, Launcher, and SharkDeck to ~/.local/
set -euo pipefail

MUGEN_VERSION="${MUGEN_VERSION:-0.1.0}"
CDN_BASE="https://cdn.mugen.gg/releases/${MUGEN_VERSION}"

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

info "${BOLD}Mugen v${MUGEN_VERSION} Installer${NC}"
echo ""

# --- Create directories ---
info "Creating directories..."
mkdir -p ~/.local/bin
mkdir -p ~/.config/mugen/apps
mkdir -p ~/.config/systemd/user
mkdir -p ~/.local/share/mugen/{launcher,apps,profiles,logs,cache}

# --- Download daemon ---
info "Downloading mugen-daemon..."
if command -v curl &>/dev/null; then
    curl -fsSL "${CDN_BASE}/mugen-daemon" -o ~/.local/bin/mugen-daemon
elif command -v wget &>/dev/null; then
    wget -q "${CDN_BASE}/mugen-daemon" -O ~/.local/bin/mugen-daemon
else
    err "curl or wget required"
    exit 1
fi
chmod +x ~/.local/bin/mugen-daemon
ok "Daemon installed"

# --- Download launcher ---
info "Downloading Mugen Launcher..."
if command -v curl &>/dev/null; then
    curl -fsSL "${CDN_BASE}/Mugen.AppImage" -o ~/.local/share/mugen/launcher/Mugen.AppImage
else
    wget -q "${CDN_BASE}/Mugen.AppImage" -O ~/.local/share/mugen/launcher/Mugen.AppImage
fi
chmod +x ~/.local/share/mugen/launcher/Mugen.AppImage
ok "Launcher installed"

# --- Download SharkDeck ---
info "Downloading SharkDeck..."
mkdir -p ~/.local/share/mugen/apps/sharkdeck
if command -v curl &>/dev/null; then
    curl -fsSL "${CDN_BASE}/sharkdeck.tar.gz" | tar xz -C ~/.local/share/mugen/apps/sharkdeck
else
    wget -q "${CDN_BASE}/sharkdeck.tar.gz" -O - | tar xz -C ~/.local/share/mugen/apps/sharkdeck
fi
# Copy manifest to config dir
cp ~/.local/share/mugen/apps/sharkdeck/cc-app.json ~/.config/mugen/apps/sharkdeck/ 2>/dev/null || true
ok "SharkDeck installed"

# --- Install systemd service ---
info "Installing systemd service..."
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
ok "Systemd service installed and started"

# --- Enable lingering (survive logout) ---
if command -v loginctl &>/dev/null; then
    loginctl enable-linger "$(whoami)" 2>/dev/null || true
    ok "User linger enabled (daemon survives reboot)"
fi

# --- Verify ---
echo ""
sleep 1
if curl -sf http://127.0.0.1:7331/health >/dev/null 2>&1; then
    ok "${BOLD}Mugen installed successfully!${NC}"
    echo ""
    info "Daemon:   http://127.0.0.1:7331/health"
    info "Launcher: ~/.local/share/mugen/launcher/Mugen.AppImage"
    info "Service:  systemctl --user status mugen.service"
else
    err "Daemon may not have started. Check: systemctl --user status mugen.service"
fi

echo ""
info "Add ~/.local/bin to PATH if not already:"
info "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc"
