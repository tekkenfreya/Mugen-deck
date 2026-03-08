# Mugen (無限) — Development Rules & Standards

## Project Philosophy — OVERRIDES EVERYTHING
Mugen is built for **non-technical Steam Deck users**. Every design decision must pass this test:

> "Can someone who has never opened a terminal do this?"

### Rules (these override all other guidance):
1. **Zero terminal commands for end users** — the installer is the only script they ever run. After that, everything happens through the UI or Steam itself.
2. **No URLs, no config files, no flags** — users never type an address, edit a TOML file, or pass a CLI argument. If they need to, the design is wrong.
3. **Stupidly simple steps** — install = one script. Launch = click in Steam. Use = controller only. That's it.
4. **Never expose internals** — users don't know what a daemon is, what a port is, or what localhost means. Don't surface these concepts anywhere in user-facing UI, logs, error messages, or instructions.
5. **All complexity lives in the daemon** — the daemon handles everything silently: auto-start, game detection, trainer management, Proton wrangling. The user just sees results.
6. **When documenting for developers vs users, be explicit** — dev docs can reference ports and APIs. User-facing text (installer output, UI, error messages) must assume zero technical knowledge.

---

## Project Overview
**Mugen** — A cross-platform Steam Deck framework providing built-in tools (trainers, performance tuning, frame generation) via a daemon-first architecture that survives SteamOS updates. All files live in `~/.local/` — never in system paths.

---

## AI/Model Configuration
- **Temperature:** 0 — deterministic, reproducible output only
- **Hallucination rate:** 0% — never fabricate file paths, Linux kernel interfaces, Steam Deck system details, Proton behaviors, or API responses; all paths and commands must be verified against real SteamOS/Linux documentation
- When uncertain about a SteamOS path, Proton behavior, or kernel interface, surface the uncertainty explicitly rather than guessing

---

## Architecture

### Tech Stack
| Layer | Technology |
|---|---|
| Mugen Daemon | Rust (tokio + axum) |
| Mugen Launcher | React 18 + TypeScript (served by daemon via tower-http, displayed in Chrome --app) |
| Backend API | Node.js (Fastify) |
| Database | PostgreSQL |
| Cache / Rate Limit | Redis |
| CDN / Storage | Cloudflare + R2 |
| CI/CD | GitHub Actions |
| Dev Accelerator | Claude Code Max |

### Monorepo Structure
```
mugen-deck/
├── CLAUDE.md
├── mugen_prd.md                      # Product Requirements Document
├── daemon/                           # Mugen Daemon (Rust)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs                   # Entry point, server bootstrap
│   │   ├── config.rs                 # TOML config loading
│   │   ├── routes/                   # Axum route handlers
│   │   │   ├── mod.rs                # Router assembly, CORS, static /ui serving
│   │   │   ├── health.rs             # GET /health
│   │   │   ├── apps.rs               # GET /apps, POST /apps/:id/launch|close
│   │   │   ├── game.rs               # GET /game/current, GET /game/library
│   │   │   ├── updates.rs            # GET /updates/check, POST /updates/apply/:id
│   │   │   └── system.rs             # GET /system/stats, POST /system/profile/:game_id
│   │   ├── game_detection.rs         # Steam process + .acf file monitoring
│   │   ├── app_manager.rs            # App lifecycle, manifest loading
│   │   ├── auth.rs                   # Session token generation/validation
│   │   └── error.rs                  # Unified error types
│   ├── tests/
│   └── mugen.service                 # Systemd user service definition
├── launcher/                         # Mugen Launcher (React SPA, served by daemon)
│   ├── package.json
│   ├── tsconfig.json
│   ├── vite.config.ts
│   └── src/                          # React frontend
│       ├── main.tsx                  # Entry point
│       ├── App.tsx                   # Root component, router
│       ├── components/               # Reusable UI components
│       │   ├── AppGrid.tsx           # App library grid view
│       │   ├── AppCard.tsx           # Single app card
│       │   ├── ControllerNav.tsx     # Controller navigation handler
│       │   ├── StatusBar.tsx         # Daemon connection status + exit button
│       │   └── TrainerCard.tsx       # Trainer display card
│       ├── pages/                    # Full-screen pages
│       │   ├── Home.tsx              # App library
│       │   ├── AppDetail.tsx         # App detail view
│       │   ├── Settings.tsx          # Settings page
│       │   └── SharkDeck.tsx         # Trainer search/download/launch UI
│       ├── hooks/                    # Custom React hooks
│       │   ├── useDaemon.ts          # Daemon API communication
│       │   ├── useController.ts      # Gamepad input handling
│       │   ├── useApps.ts            # App state management
│       │   └── useTrainers.ts        # Trainer state management
│       ├── api/                      # API client layer
│       │   └── daemon.ts             # Typed daemon REST client
│       ├── types/                    # TypeScript type definitions
│       │   └── index.ts
│       └── styles/                   # Global styles
│           └── global.css
├── backend/                          # Mugen Backend (Fastify)
│   ├── package.json
│   ├── tsconfig.json
│   └── src/
│       ├── server.ts                 # Entry point, Fastify bootstrap
│       ├── routes/                   # Route handlers
│       │   └── apps.ts              # GET /api/v1/apps/:id/latest
│       └── config.ts                # Environment config
├── installer/                        # One-command installer
│   └── install.sh                   # curl -L mugen.gg/install | bash
├── apps/                             # Built-in Mugen apps
│   └── sharkdeck/                   # SharkDeck — Trainer Manager
│       ├── cc-app.json              # App manifest with permissions
│       ├── package.json
│       └── src/
│           ├── index.ts             # Entry point
│           ├── fling.ts             # Fling trainer database scraper
│           ├── trainer.ts           # Trainer download + cache management
│           ├── proton.ts            # Proton discovery, isolated prefix, trainer launch
│           └── types.ts             # Trainer type definitions
└── scripts/                          # Development & build scripts
    ├── dev.sh                       # Start all services for development
    └── build.sh                     # Production build script
```

---

## Coding Standards

### Rust (Daemon)
- **Edition:** 2021, MSRV 1.75+
- **Async runtime:** `tokio` (multi-threaded)
- **HTTP framework:** `axum`
- **Serialization:** `serde` + `serde_json` for JSON, `toml` for config
- **Error handling:** Use `thiserror` for custom error types, `anyhow` for application errors
- **No `unwrap()` in production code** — use `?` operator or explicit error handling
- **No `unsafe` blocks** unless absolutely necessary and commented with safety justification
- Run `cargo clippy -- -D warnings` — zero warnings tolerated
- Run `cargo fmt` — all code must be formatted
- All public functions must have doc comments (`///`)
- Log using `tracing` crate — never `println!()` in production
- Log levels: `DEBUG` for internal state, `INFO` for lifecycle events, `WARN` for recoverable issues, `ERROR` for failures

### TypeScript (Launcher + Backend + Apps)
- **TypeScript strict mode** — `"strict": true` in all `tsconfig.json`
- **No `any` type** — use `unknown` and type guards instead
- **No non-null assertions (`!`)** — handle null/undefined explicitly
- ESLint + Prettier enforced — zero warnings tolerated
- Use `const` by default; `let` only when reassignment is needed; never `var`
- All API responses must be typed — no raw `fetch` without type validation
- Prefer named exports over default exports
- Use absolute imports via path aliases (`@/components/...`)

### React (Launcher)
- Functional components only — no class components
- Use React hooks for all state and effects
- Component files: one component per file, filename matches component name (PascalCase)
- Props interfaces defined in the same file, named `{ComponentName}Props`
- No inline styles — use CSS modules or Tailwind utility classes
- All user-facing text must be hardcoded strings (no i18n in Phase 1)

---

## Daemon Specifics

### REST API (`localhost:7331`)
- All responses are JSON with consistent envelope: `{ "ok": true, "data": ... }` or `{ "ok": false, "error": "..." }`
- Session token required in `Authorization: Bearer <token>` header for all endpoints except `GET /health`
- Token generated at daemon startup, rotated every 24 hours, stored in `~/.config/mugen/session.token`
- CORS: reject all origins via `tower-http::CorsLayer` — daemon is localhost-only
- Bind to `127.0.0.1` only — never `0.0.0.0`
- Serves launcher React SPA at `/ui` via `tower-http::ServeDir` from `~/.local/share/mugen/launcher/ui/`

### Game Detection
- Monitor Steam processes via `/proc` filesystem
- Parse `.acf` files at `~/.local/share/Steam/steamapps/` for AppID, game name, install path
- Poll interval: 5 seconds
- Cache library scan results; invalidate on `.acf` file modification time change

### Installation Paths (SteamOS)
All Mugen files live in the user home directory. **Nothing is installed to system paths.**
```
~/.local/bin/mugen-daemon              # Daemon binary
~/.config/mugen/config.toml           # Daemon configuration
~/.config/mugen/session.token         # Current session token
~/.config/mugen/apps/                 # Registered app manifests
~/.config/systemd/user/mugen.service  # Systemd user service
~/.local/share/mugen/launcher/ui/     # Launcher React SPA (built files served by daemon)
~/.local/bin/mugen-launcher           # Chrome --app wrapper script
~/.local/share/mugen/apps/            # Installed app bundles
~/.local/share/mugen/profiles/        # Per-game profile data
~/.local/share/mugen/logs/            # Daemon and app logs
~/.local/share/mugen/cache/           # Trainer cache, metadata cache
~/.config/mugen-chrome/               # Isolated Chrome profile for launcher
```

---

## Launcher Specifics

### Architecture (Chrome --app)
- **How it works:** Daemon serves the React SPA via `tower-http::ServeDir` at `/ui`. Chrome Flatpak opens `http://127.0.0.1:7331/ui/` in `--app` mode (chromeless window). Gamescope handles fullscreen in Gaming Mode automatically.
- **Why not Tauri:** WebKitGTK is not installed on SteamOS and EGL crashes even after manual install
- **Why not Electron:** Steam Runtime strips `DISPLAY` env var and `libcups.so.2` — Electron window never appears in Gaming Mode
- **Launcher wrapper:** `~/.local/bin/mugen-launcher` shell script that runs `flatpak run com.google.Chrome --app=http://127.0.0.1:7331/ui/ --user-data-dir=/home/deck/.config/mugen-chrome`
- **Chrome flags to avoid:** `--kiosk` (traps user), `--start-fullscreen` (crashes in Gaming Mode), `--no-first-run` (prevents launch)

### Controller Navigation
- D-pad: navigate between focusable elements
- A button (gamepad button 0): select/confirm
- B button (gamepad button 1): back/cancel
- Use the Gamepad API (`navigator.getGamepads()`)
- Focusable elements must have visible focus indicators
- Navigation must work without keyboard or mouse connected

### UI Design
- Dark theme optimized for Steam Deck's 7" 1280x800 display
- Minimum touch target size: 48x48px
- Font: system sans-serif, minimum 16px for interactive elements
- Card-based grid layout for app library
- Smooth transitions between pages (no hard cuts)

---

## SharkDeck Specifics

### Trainer Flow
1. Daemon detects currently running game via game detection
2. SharkDeck queries Fling trainer database for matching game
3. User selects trainer → SharkDeck downloads to `~/.local/share/mugen/cache/trainers/`
4. Trainer launched alongside game through Proton in an **isolated prefix**
5. Per-game trainer profiles saved for future sessions

### Security
- Trainers run in isolated Proton prefixes — separate from the game's prefix
- Network access blocked by default for trainer processes
- Downloaded trainers verified via checksum before execution

---

## Backend Specifics (Phase 1 — Minimal)

### Fastify Configuration
- TypeScript with strict mode
- JSON Schema validation on all route inputs
- Structured logging via Fastify's built-in pino logger
- CORS: allow `mugen.gg`, `cheatcode.dev`, `localhost` (dev only)

### Phase 1 Endpoints
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/apps/:id/latest` | Get latest version info + download URL for an app |
| GET | `/api/v1/health` | Backend health check |

---

## Testing Standards

### Rust (Daemon)
- Use `#[cfg(test)]` module-level tests + integration tests in `tests/` directory
- Mock filesystem and process interactions — no real Steam installation required
- Run: `cargo test`
- Target: 70%+ coverage on business logic

### TypeScript (Launcher + Backend + Apps)
- Use `vitest` for unit tests
- Mock all daemon API calls — no real daemon required for launcher tests
- Mock all HTTP calls in backend tests
- Run: `npm test` or `npx vitest`
- Target: 70%+ coverage on business logic (excluding UI components in Phase 1)

---

## Build & Development

### Development Setup
```bash
# Daemon (from daemon/)
cargo run

# Launcher (from launcher/)
npm install && npm run dev    # Vite dev server with HMR

# Backend (from backend/)
npm install && npm run dev

# SharkDeck (from apps/sharkdeck/)
npm install && npm run dev
```

### Production Build
**IMPORTANT: The Rust daemon MUST be built inside WSL** — cross-compiling from Windows fails because the `ring` crate requires `x86_64-linux-gnu-gcc`. Always use WSL for daemon builds.

```bash
# Daemon — build in WSL (login shell required for cargo)
wsl bash -lc 'cd "/mnt/c/Users/Egofoxxx/Documents/Development Area/mugen-deck/daemon" && cargo build --release'

# Launcher (can build on Windows or WSL)
cd launcher && npm run build  # Produces dist/ with static files

# Copy artifacts to sharkdeck-install/
cp daemon/target/release/sharkdeck-daemon sharkdeck-install/
cp -r launcher/dist/* sharkdeck-install/ui/

# Backend
npm run build
```

---

## Git Conventions
- Branch naming: `feature/`, `fix/`, `chore/`
- Commit format: `type(scope): short description` — e.g., `feat(daemon): add game detection via .acf parsing`
- Scopes: `daemon`, `launcher`, `backend`, `sharkdeck`, `installer`, `docs`
- Never commit: credentials, `.env`, `target/`, `dist/`, `node_modules/`, `*.AppImage`
- `.gitignore` must exclude all of the above
- Semantic versioning, git tags for all releases

---

## Security Rules
- Daemon binds to `127.0.0.1:7331` only — never accessible from outside the device
- Session token rotates every 24 hours
- All apps sandboxed to their own directories
- No string concatenation in SQL queries — parameterized only
- All user input validated server-side
- Secrets in environment variables only — never in code or git
- Trainer processes network-blocked by default

---

## Steam Deck Verified Paths (SteamOS 3.x)
These are confirmed real paths — do not invent alternatives:
| Purpose | Path |
|---|---|
| Steam library | `~/.local/share/Steam` |
| Steam apps | `~/.local/share/Steam/steamapps/` |
| App manifest files | `~/.local/share/Steam/steamapps/appmanifest_*.acf` |
| Proton compatdata | `~/.local/share/Steam/steamapps/compatdata/` |
| Proton versions | `~/.local/share/Steam/compatibilitytools.d/` |
| Flatpak apps | `~/.var/app` |
| SD card mount | `/run/media/` |
| User systemd services | `~/.config/systemd/user/` |
| User binaries | `~/.local/bin/` |
| User data | `~/.local/share/` |
| User config | `~/.config/` |

---

## Phase 1 Scope (Proof of Concept)
**In scope:**
- Mugen Daemon (Rust) — systemd service, REST API, game detection
- Mugen Launcher (React SPA + Chrome --app) — fullscreen via Gamescope, controller nav, app grid
- SharkDeck Phase 1 — detect game, download trainer, launch via Proton
- Backend minimal — app version endpoint only
- Installer script — one-command install

**Out of scope:**
- User accounts and authentication
- Cloud sync
- Mugen Pro / subscriptions
- LosslessDeck, PowerMugen, SuspendDeck, GameRadar
- Mobile companion app
- Analytics
- Decky plugin

---

*Mugen — Vectrx — Confidential — March 2026*
