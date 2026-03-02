# Steam Deck Deployment — Lessons Learned

## What DOESN'T work on SteamOS Gaming Mode

### Tauri
WebKitGTK is not installed on SteamOS. Even after manual install (`pacman -S webkit2gtk-4.1`), EGL crashes on launch (`Could not create default EGL display: EGL_BAD_PARAMETER`). SteamOS read-only filesystem must be unlocked with `sudo steamos-readonly disable` for pacman, and gets reset on SteamOS updates. **Not viable.**

### Electron AppImage
Electron bundles Chromium so it avoids the WebKitGTK issue. Works in Desktop Mode with `--no-sandbox`. However, in Gaming Mode:
- Steam Runtime strips the `DISPLAY` environment variable before launching non-Steam games
- Electron needs `DISPLAY` set before process start — setting it in `main.cjs` is too late
- `libcups.so.2` is also stripped by Steam Runtime — bundling it doesn't help
- Process runs (visible in `ps aux`) but Gamescope never sees the window
- Setting `DISPLAY=:0` or `DISPLAY=:1` via launch options doesn't help

**Not viable for Gaming Mode.**

### Shell scripts as non-Steam games
Steam's reaper does not reliably execute shell script wrappers added as non-Steam games in Gaming Mode. Log files from wrapper scripts were never created, indicating the script body never runs.

### Chrome flags that break things
| Flag | Problem |
|------|---------|
| `--kiosk` | Traps user in fullscreen — no way to exit on Deck without force restart |
| `--start-fullscreen` | Crashes on launch in Gaming Mode |
| `--no-first-run` | Causes Chrome to silently not launch at all in Gaming Mode |

---

## What WORKS: Daemon + Chrome Flatpak --app

### Architecture
1. **Daemon** (Rust/axum) serves the React SPA via `tower-http::services::ServeDir` at `/ui`
2. **Chrome Flatpak** (`com.google.Chrome`) opens `http://127.0.0.1:7331/ui/` in `--app` mode (chromeless window)
3. **Gamescope** handles fullscreen automatically in Gaming Mode — never force fullscreen flags

### Why Chrome Flatpak works
- Valve ensures browser Flatpaks are compatible with Gamescope
- Chrome Flatpak manages its own display connection — doesn't rely on Steam Runtime's stripped environment
- `--app` mode gives a chromeless window that looks like a native app
- Gamescope stretches it to fullscreen automatically

### Working launcher script
```bash
#!/bin/bash
exec flatpak run com.google.Chrome \
  --app=http://127.0.0.1:7331/ui/ \
  --user-data-dir=/home/deck/.config/mugen-chrome
```

Key points:
- `--user-data-dir` isolates Mugen's Chrome profile from user's regular Chrome
- No `--kiosk`, `--start-fullscreen`, or `--no-first-run`
- First-run popups (analytics, default browser) show once then go away

---

## Installation

### Install paths
```
/home/deck/.local/bin/mugen-daemon           # Daemon binary (~4MB)
/home/deck/.local/bin/mugen-launcher         # Chrome --app wrapper script
/home/deck/.local/share/mugen/launcher/ui/   # React SPA dist files (~160KB)
/home/deck/.config/systemd/user/mugen.service  # Systemd user service
/home/deck/.config/mugen/                    # Daemon config + session token
/home/deck/.config/mugen-chrome/             # Isolated Chrome profile
```

### Installer flow
1. Stop existing daemon if running
2. Copy daemon binary to `~/.local/bin/`
3. Copy UI files to `~/.local/share/mugen/launcher/ui/`
4. Copy launcher wrapper to `~/.local/bin/`
5. Install systemd user service and enable it
6. Start daemon
7. User manually adds `mugen-launcher` as non-Steam game

### WARNING: shortcuts.vdf
**NEVER programmatically modify `~/.local/share/Steam/config/shortcuts.vdf`**. It's a binary file with a non-trivial format. A Python script that attempted to auto-add Mugen to Steam overwrote all existing non-Steam game shortcuts. Always instruct users to manually add via "Add a Non-Steam Game" in Steam.

---

## Debugging on Deck

### SSH access
```bash
# On Deck: enable SSH
sudo systemctl start sshd

# From PC:
ssh deck@<deck-ip>
```

### Common operations
```bash
# Check daemon status
systemctl --user status mugen.service

# View daemon logs
journalctl --user -u mugen.service --no-pager -n 50

# Restart daemon
systemctl --user restart mugen.service

# Kill stuck Chrome
pkill -9 -f "chrome.*mugen"

# Test daemon health
curl http://127.0.0.1:7331/health
```

### SteamOS read-only filesystem
SteamOS uses a read-only root filesystem. To install system packages:
```bash
sudo steamos-readonly disable
sudo pacman-key --init && sudo pacman-key --populate
sudo pacman -S <package>
sudo steamos-readonly enable
```
Note: System packages are wiped on SteamOS updates. Prefer Flatpak or user-space binaries.

---

## File transfer
- **DeckBridge** — GUI tool for PC→Deck file transfer
- **SCP** — `scp -r ./mugen-install/ deck@<ip>:~/Downloads/`
- Long commands break in Deck's virtual keyboard — write files on PC and transfer
