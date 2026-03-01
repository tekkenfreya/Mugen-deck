# MUGEN — 無限のDeck
**Infinite Tools. Every Device.**

> Product Requirements Document — Version 1.0 | Vectrx | March 2026

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [The Problem](#2-the-problem)
3. [The Mugen Solution](#3-the-mugen-solution)
4. [Technical Architecture](#4-technical-architecture)
5. [Built-In Tools](#5-built-in-tools)
6. [Monetization](#6-monetization)
7. [Infrastructure](#7-infrastructure)
8. [Security](#8-security)
9. [Development Approach](#9-development-approach)
10. [Phase 1 — Proof of Concept](#10-phase-1--proof-of-concept)
11. [Full Roadmap](#11-full-roadmap)
12. [Risks and Mitigations](#12-risks-and-mitigations)
- [Appendix A — Directory Structure](#appendix-a--directory-structure)
- [Appendix B — Mugen vs Decky Comparison](#appendix-b--mugen-vs-decky-comparison)

---

## 1. Executive Summary

Mugen (無限 — meaning "unlimited" in Japanese) is a cross-platform framework for Steam Deck power users. It provides a suite of built-in tools — trainer management, performance tuning, frame generation control, and more — all from a single stable launcher that survives SteamOS updates.

Unlike Decky Loader, which injects into Steam's browser UI and breaks on every major SteamOS update, Mugen lives entirely inside the user's home directory (`~/.local/`) and communicates through a background daemon. This makes it structurally immune to system updates.

Mugen is built and maintained by Vectrx, a 2-person software studio, using **Claude Code Max** as the primary development accelerator. The project is designed to be the only tool a Steam Deck power user ever needs.

| Metric | Target |
|--------|--------|
| Launch Platform | Steam Deck (SteamOS) |
| Phase 1 Timeline | 5 days (Claude Code Max) |
| Monetization | Free tier + Mugen Pro ($4/month) |
| Break-even | ~30 Pro subscribers |
| Year 1 Revenue Target | ₱80,000/month |
| Development Tool | Claude Code Max (Anthropic) |
| Team Size | 2 (Vectrx) |

---

## 2. The Problem

### 2.1 Decky Loader — Great UX, Fragile Foundation

Decky Loader is the dominant Steam Deck plugin platform. It injects plugins into Steam's Quick Access Menu (QAM) — the overlay you get when pressing `...` on the controller. This is excellent UX: users never leave their game to access tools.

The fundamental problem is how Decky works. Steam's Gaming Mode UI is built on CEF (Chromium Embedded Framework) — essentially a web browser running React. Decky reverse-engineers Steam's internal JavaScript to inject plugins into that browser. When Valve updates Steam internally, Decky's injection points break.

- Decky breaks on every major SteamOS update
- Plugins installed to `/usr` paths get wiped when SteamOS updates the root partition
- The Decky team must manually patch after every breaking update
- Users are left without tools until patches ship

### 2.2 The Fragmented Tool Ecosystem

Beyond stability, the current ecosystem is fragmented. Power users need multiple separate tools:

- **PowerTools** — CPU/GPU performance tuning
- **lsfg-vk Decky plugin** — frame generation (Lossless Scaling on Deck)
- **CheatDeck** — trainer management *(broken since November 2025, unresolved)*
- **ProtonDB Badges** — compatibility info
- **CSS Loader** — UI theming

There is no unified platform that manages all of these, provides cloud sync across devices, or offers a companion mobile app for remote control.

---

## 3. The Mugen Solution

### 3.1 Core Philosophy

Mugen takes a daemon-first approach. A lightweight background service (Mugen Daemon) runs as a systemd user service, living entirely in `~/.local/` — a directory that is **never touched by SteamOS updates**. All apps communicate through this daemon. The daemon exposes a local REST API on `localhost:7331`.

> **KEY INSIGHT:** SteamOS has a read-only root filesystem. Updates replace `/usr` and `/etc` but NEVER touch `/home/deck/`. Mugen lives entirely in `~/.local/`, making it structurally immune to SteamOS updates.

### 3.2 Decky Integration Strategy

Rather than competing with Decky, Mugen uses a **"doorbell" strategy**:

- A tiny Decky plugin (~50 lines) lives in the Quick Access Menu as an entry point
- Tapping it launches the full Mugen Launcher window (a separate Tauri app)
- If Decky breaks on a SteamOS update, Mugen still works — users just launch it from the Steam library directly
- The Decky plugin is optional convenience, not a dependency

This gives users the native `...` button feel while keeping all actual functionality outside Steam's fragile browser environment.

### 3.3 Built-in Tools

Mugen ships with a curated set of built-in tools, replacing the need for multiple separate Decky plugins:

| Tool | What It Does | Replaces |
|------|-------------|----------|
| **SharkDeck** | Cheat/trainer manager — detects current game, downloads from Fling, launches via Proton | CheatDeck (broken) |
| **LosslessDeck** | Frame generation control — install/update lsfg-vk, FPS multiplier 2x/3x/4x, flow scale, performance mode, HDR, **per-game profiles** | lsfg-vk Decky plugin |
| **PowerMugen** | Performance tuning — CPU governor, SMT, GPU clock limits, TDP, per-game profiles | PowerTools |
| **SuspendDeck** | Freeze/resume any game instantly via SIGSTOP/SIGCONT signals | Pause Games plugin |
| **GameRadar** | Per-game profile manager — applies settings automatically when a game launches | Manual config switching |

---

## 4. Technical Architecture

### 4.1 System Overview

| Component | Technology | Location | Purpose |
|-----------|-----------|----------|---------|
| Mugen Daemon | Rust | `~/.local/bin/mugen-daemon` | Core service, REST API, game detection, app lifecycle |
| Mugen Launcher | Tauri + React + TypeScript | `~/.local/share/mugen/launcher/` | Gaming Mode UI, controller navigation, app hub |
| Decky Plugin | React + TypeScript | Decky standard path | Quick Access Menu shortcut only |
| Mugen Backend | Node.js (Fastify) | Hetzner CX32 VPS | Auth, cloud sync, update distribution, app registry |
| cheatcode.dev | Next.js | Hetzner CX32 VPS | Website, downloads, blog, AdSense |
| Mugen Mobile | React Native | iOS + Android | Remote control companion app (Phase 3) |

### 4.2 Mugen Daemon

The daemon is the backbone of the entire system. It runs as a systemd user service and starts automatically on boot.

**Installation Paths:**
```
~/.local/bin/mugen-daemon          # Binary
~/.config/mugen/config.toml       # Config
~/.config/mugen/apps/             # App manifests
~/.config/systemd/user/mugen.service  # Service definition
~/.local/share/mugen/             # Data, logs, cache
```

**REST API Endpoints (`localhost:7331`):**

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Daemon status, version, uptime |
| GET | `/apps` | List registered apps |
| POST | `/apps/:id/launch` | Launch an app |
| POST | `/apps/:id/close` | Close an app |
| GET | `/game/current` | Currently running Steam game |
| GET | `/game/library` | Full Steam library scan |
| GET | `/updates/check` | Check for app/daemon updates |
| POST | `/updates/apply/:id` | Apply an update |
| GET | `/system/stats` | CPU, GPU, RAM, temps |
| POST | `/system/profile/:game_id` | Apply per-game profile |

**Game Detection:** The daemon monitors Steam process information and reads Steam's `.acf` (Application Cache Files) at `~/.local/share/Steam/steamapps/`. This gives it the AppID, game name, and install path in real time.

### 4.3 Mugen Launcher

A full-screen Tauri application optimized for controller navigation. Added to Steam as a non-Steam game.

- Full screen, no window decorations, not resizable
- Controller navigation: D-pad to move, A to select, B to go back
- Screens: App library (grid view), App detail, Settings, Update log
- Communicates entirely with daemon via `localhost:7331`
- Never interacts with Steam's internal browser

### 4.4 Why the Launcher Survives Updates

Tauri bundles the app as a single AppImage stored in `~/.local/share/mugen/launcher/`. AppImages are self-contained executables — no system dependencies, no installation. The Steam library entry points to this file path. Since `~/.local/` is never touched by SteamOS updates, the launcher is always there.

---

## 5. Built-In Tools

### 5.1 SharkDeck — Trainer Manager

The trainer/cheat manager for Steam Deck. The only working alternative to CheatDeck (which broke in November 2025 and has not been fixed).

**How It Works:**
1. Daemon detects currently running game (or user browses library)
2. SharkDeck queries Fling trainer database for matching game
3. User selects trainer, SharkDeck downloads it
4. Trainer launched alongside game through Proton in an isolated prefix
5. Per-game trainer profiles saved for future sessions

**Trainer Sources:**
- Primary: Fling Trainers (flingtrainer.com) — gold standard, thousands of free trainers since 2012
- Community: User-submitted trainers via Mugen backend (Phase 2)
- Per-trainer Proton version profiles to handle compatibility differences

**Security:**
- Trainers run in isolated Proton prefixes, separate from the game's prefix
- Network access blocked by default for trainer processes
- Community trainer submissions require code signing verification

### 5.2 LosslessDeck — Frame Generation Control

A configurator for lsfg-vk (the Vulkan implementation of Lossless Scaling's frame generation). Mugen does not re-implement frame generation — it installs, updates, and configures the existing lsfg-vk binary.

**Features:**
- One-click install and auto-update of lsfg-vk
- FPS multiplier: 2x, 3x, 4x
- Flow scale slider (quality vs. performance)
- Performance mode toggle
- HDR toggle
- **Per-game profiles — automatically apply preferred settings when a game launches**

> **ADVANTAGE:** Per-game profiles is a key differentiator. The existing Decky lsfg-vk plugin has no per-game profiles — users must manually adjust every time they switch games. Mugen remembers settings per game.

### 5.3 PowerMugen — Performance Tuner

A replacement for PowerTools. Talks directly to Linux kernel interfaces — no Steam injection required.

- CPU governor (performance / powersave / balanced)
- SMT (Simultaneous Multi-Threading) toggle
- GPU clock speed limits
- TDP (Thermal Design Power) limits
- Fan curve control via `/sys/class/thermal/`
- Per-game profiles via GameRadar integration

### 5.4 SuspendDeck — Game Suspension

Instantly freeze and resume any game using POSIX signals.

- `SIGSTOP` to freeze the game process completely
- `SIGCONT` to resume
- Zero CPU usage while suspended
- Accessible from the Mugen Launcher without closing the game

### 5.5 GameRadar — Per-Game Profile Manager

The glue that ties everything together. Auto-applies per-game settings when the daemon detects a game launch.

- Per-game: FPS multiplier (LosslessDeck), CPU/GPU limits (PowerMugen), trainer profile (SharkDeck)
- User can import/export profiles
- Cloud sync with Mugen Pro

---

## 6. Monetization

### 6.1 Philosophy

The free tier is never crippled. All core tools work fully forever without payment. Mugen Pro adds cloud convenience — sync, early access, and remote control — not gating features that should be free.

### 6.2 Tier Comparison

| Feature | Free | Mugen Pro ($4/month) |
|---------|------|----------------------|
| All built-in tools | Full access | Full access |
| Per-game profiles | Local storage | Cloud synced (E2E encrypted) |
| Mobile companion app | View-only | Full remote control |
| Early access to new tools | No | Yes |
| Vote on next tool to build | No | Yes |
| Priority update queue | No | Yes |
| Website ads | Ads shown | Ad-free |
| Discord badge | No | Pro badge |

### 6.3 Revenue Streams

| Source | Timing | Target (Month 12) |
|--------|--------|-------------------|
| Mugen Pro subscriptions | Month 2+ | ₱80,000/month (500 subs) |
| Google AdSense (cheatcode.dev) | Month 1+ | ₱8,000/month |
| YouTube ad revenue | Month 4+ (after monetization) | ₱20,000/month |
| Patreon | Month 3+ | ₱15,000/month |
| YouTube sponsorships | Month 8+ | ₱60,000/month |

### 6.4 Payment Processing

- **Stripe** — international cards, subscription billing
- **Paymongo** — PH users (GCash, Maya, local cards)
- **Stripe Billing** — subscription management, dunning, grace periods
- 3-day grace period on failed payments before downgrade

---

## 7. Infrastructure

### 7.1 Server Architecture

| Component | Provider | Spec | Cost |
|-----------|----------|------|------|
| Primary VPS | Hetzner CX32 | 4 vCPU, 8GB RAM, 80GB SSD, 20TB traffic | €6.80/month |
| Database server | Hetzner CX22 (when >5k users) | 2 vCPU, 4GB RAM, 40GB SSD | €3.79/month |
| CDN + DDoS protection | Cloudflare | Free tier | €0 |
| App binary storage | Cloudflare R2 | S3-compatible, 10GB + 1M req free | €0–2/month |
| DNS | Cloudflare | Free | €0 |

### 7.2 Server Stack

```
Internet → Cloudflare → Hetzner CX32
                         ├── Nginx (reverse proxy + SSL)
                         │   ├── /api/* → Fastify (Node.js, port 3001)
                         │   ├── /downloads → R2 redirect
                         │   └── /* → Next.js (cheatcode.dev, port 3000)
                         ├── PostgreSQL (localhost only)
                         ├── Redis (localhost only)
                         └── PM2 (process manager)
```

### 7.3 Capacity at Launch

- ~2,000 concurrent users on CX32
- ~10,000 downloads/day
- ~500,000 API requests/day
- **Break-even at ~30 Pro subscribers** — achievable within first month of launch

---

## 8. Security

### 8.1 Authentication

- JWT access tokens: 15-minute expiry
- Refresh tokens: 30-day expiry, httpOnly cookies, rotated on every use
- Passwords: bcrypt cost factor 12, minimum 8 characters
- HaveIBeenPwned check on registration
- Rate limiting: 5 login attempts per 15 minutes per IP
- Account lockout: 30 minutes after 10 failed attempts

### 8.2 Daemon Security

- Binds to `127.0.0.1:7331` only — never accessible from outside the device
- Session token generated at startup, rotates every 24 hours
- All apps sandboxed to their own directories
- Permissions declared in app manifest, enforced at runtime
- Code signing: all apps signed with Vectrx private key, verified before install
- Trainer processes run in isolated Proton prefixes, network blocked by default

### 8.3 API Security

- TLS 1.3 minimum, HSTS 1-year max-age
- Certificate pinning in launcher and mobile app
- All input validated server-side via JSON schema
- Parameterized SQL only — no string concatenation queries
- Rate limiting via Redis sliding window: 100 req/min per user, 5/min for auth endpoints
- CORS strict whitelist: `cheatcode.dev`, `mugen.gg`, `localhost` (dev only)

### 8.4 Data Security

- PostgreSQL encrypted at rest (Hetzner volume encryption)
- Cloud sync data: end-to-end encrypted — server sees only encrypted blobs
- Encryption key derived from user password via PBKDF2 — Vectrx cannot decrypt user data
- Daily automated backups, 30-day retention
- GDPR compliant: full data export and deletion on request

### 8.5 Infrastructure Hardening

- SSH key-only authentication, password login disabled
- SSH port changed from default 22
- UFW firewall: only ports 80, 443, SSH allowed inbound
- Fail2ban for SSH brute-force protection
- Automatic security updates enabled
- All application processes run as non-root user
- Secrets in environment variables only, never in code or git

---

## 9. Development Approach

### 9.1 Claude Code Max

Mugen is built using **Claude Code Max** (Anthropic's agentic coding assistant) as the primary development accelerator. Claude Code Max removes the code-writing bottleneck, allowing the 2-person Vectrx team to ship at a pace that would otherwise require 5–10 developers.

> **CAPABILITY:** Claude Code Max can write, test, debug, and iterate across the full stack — Rust daemon, TypeScript launcher, Node.js backend, React Native mobile — in parallel. This is the core reason a 5-day Phase 1 is realistic.

**Claude Code Max usage by component:**

- **Mugen Daemon (Rust):** Generate boilerplate, API routes, game detection logic, systemd service config
- **Mugen Launcher (Tauri/React):** Component scaffolding, controller navigation logic, API integration
- **Backend (Fastify):** Auth flows, database schemas, API endpoints, Redis integration
- **SharkDeck:** Fling scraper, trainer download/launch logic, Proton prefix management
- **LosslessDeck:** lsfg-vk installer, settings UI, profile system
- **Tests:** Unit, integration, and E2E test generation across all components

### 9.2 Tech Stack

| Layer | Technology | Reason |
|-------|-----------|--------|
| Mugen Daemon | Rust | Zero overhead, system-level access, memory safe, never crashes |
| Mugen Launcher | Tauri + React + TypeScript | ~10MB binary vs Electron's 150MB, native OS integration |
| Backend API | Node.js (Fastify) | Fast, familiar, great ecosystem for rapid development |
| Website | Next.js | SSR for SEO, React familiarity |
| Mobile App | React Native | Single codebase for iOS + Android |
| Database | PostgreSQL | Reliable, powerful, great for complex queries |
| Cache / Rate Limit | Redis | Fast in-memory, built-in expiry |
| CDN / Storage | Cloudflare + R2 | Free tier covers launch, global edge |
| Payments | Stripe + Paymongo | Global cards + PH local (GCash, Maya) |
| Dev Accelerator | **Claude Code Max** | Full-stack code generation, test writing, debugging |
| CI/CD | GitHub Actions | Free for public repos, integrates with everything |

### 9.3 Development Standards

- TypeScript strict mode throughout all TypeScript projects
- ESLint + Prettier enforced via pre-commit hooks
- Rust: clippy enforced, no `unwrap()` in production code
- Minimum 70% test coverage for business logic
- `npm audit` + `cargo audit` in CI pipeline
- Semantic versioning, git tags for all releases, CHANGELOG.md maintained
- CI/CD pipeline: Build → Test → Security scan → Staging → Manual approve → Production

---

## 10. Phase 1 — Proof of Concept

### 10.1 Goal

Validate that the Mugen framework architecture works on a real Steam Deck before building the full ecosystem. Five days using Claude Code Max.

> **DEFINITION OF DONE:** Fresh Steam Deck → one-command installer → Gaming Mode → SharkDeck working → simulate SteamOS update → everything still works.

### 10.2 Six Validation Tests

| # | Test | Pass Criteria |
|---|------|--------------|
| 1 | Daemon survives reboot | Daemon running after Steam Deck restart without user action |
| 2 | Daemon survives SteamOS update | Daemon still active after `steamos-readonly` toggle + update simulation |
| 3 | Launcher opens in Gaming Mode | Tauri window launches from Steam library with correct full-screen display |
| 4 | Controller navigation works | D-pad, A button, B button all navigate correctly without keyboard/mouse |
| 5 | SharkDeck detects game + launches trainer | End-to-end: detect game → find trainer → download → launch via Proton |
| 6 | Auto-update works | Daemon detects new version from backend, downloads, installs without user action |

### 10.3 Day-by-Day Plan

**Day 1 — Mugen Daemon (Rust)**
- Systemd user service setup
- REST API: `/health`, `/apps`, `/apps/:id/launch`, `/game/current`, `/updates/check`, `/updates/apply/:id`
- Game detection via Steam process monitoring and `.acf` files
- Session token generation (rotates every 24 hours)
- Logging to `~/.local/share/mugen/logs/daemon.log`

**Day 2 — Mugen Launcher (Tauri + React)**
- Full-screen Gaming Mode window configuration
- Controller navigation (D-pad, A, B)
- App library grid view
- API integration with daemon
- Non-Steam game entry in Steam library

**Day 3 — SharkDeck Phase 1**
- Read current game from daemon
- Fling trainer database scraper
- Trainer download and Proton launch
- App manifest (`cc-app.json`) with permissions

**Day 4 — Backend + Installer**
- Minimal backend: `GET /api/v1/apps/:id/latest` endpoint
- App binaries on Cloudflare R2
- One-command installer script: `curl -L mugen.gg/install | bash`
  - Downloads daemon binary
  - Creates config directories
  - Installs systemd user service
  - Downloads launcher AppImage
  - Adds launcher to Steam library (edits VDF file)
  - Downloads and registers SharkDeck

**Day 5 — Testing and Validation**
- Run all 6 validation tests on real Steam Deck
- Simulate SteamOS update and verify survival
- Fix any issues found
- Document results

### 10.4 Phase 1 Out of Scope

- User accounts and authentication
- Cloud sync
- Mugen Pro / subscriptions
- Full Mugen Store UI
- Mobile companion app
- LosslessDeck, PowerMugen, SuspendDeck, GameRadar
- Analytics

---

## 11. Full Roadmap

| Phase | Timing | Deliverables |
|-------|--------|-------------|
| **Phase 1: Validate** | Week 1 (5 days) | Mugen Daemon, Launcher, SharkDeck Phase 1, Backend minimal, Installer. Validation: daemon survives SteamOS update. |
| **Phase 2: Launch** | Week 2–3 | SharkDeck v1.0, LosslessDeck, PowerMugen, SuspendDeck, GameRadar. Backend: full auth, app registry, user accounts. Cloud sync for Pro. Mugen Pro subscription live. cheatcode.dev full website. AdSense. Public launch r/SteamDeck. |
| **Phase 3: Grow** | Month 2–3 | Mugen Mobile MVP (React Native). YouTube channel launch. YouTube monetization milestone. Patreon launched. DeckBridge integrated into Mugen. |
| **Phase 4: Ecosystem** | Month 4–6 | Third-party developer program. Developer SDK published. 6+ tools in Mugen Store. YouTube sponsorships. dev.mugen.gg developer portal. |
| **Phase 5: Scale** | Month 6–12 | Community tool submissions. Separate database server. Windows PC launcher evaluation. 10,000 installs milestone. |

---

## 12. Risks and Mitigations

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Valve changes home directory handling in SteamOS | Very Low | `~/.local/` is a Linux standard; changing this would break thousands of apps. Monitor SteamOS changelogs. |
| Systemd user service breaks | Low | Phase 1 specifically validates this. Test on every SteamOS major version. |
| Fling Trainers blocks scraping | Medium | Add XiaoXing and community submissions from day one. Multiple sources. |
| Proton breaks trainer compatibility | Medium | Per-trainer Proton version profiles. Let users override. |
| Low adoption at launch | Medium | YouTube devlog builds audience before launch. Reddit r/SteamDeck post timed with launch. |
| Decky team copies daemon-first approach | Low | Cross-platform + cloud sync takes months to replicate. First-mover advantage. |
| Server costs exceed revenue | Low | Break even at ~30 Pro subs on CX32. Hetzner is extremely affordable. |
| Claude Code Max outage during development | Low | Local progress saved in git. Resume when available. Edson can code manually for simple tasks. |

---

## Appendix A — Directory Structure

All Mugen files install to the user home directory. Nothing is installed to system paths.

```
~/.local/bin/mugen-daemon              # Mugen Daemon binary (Rust)
~/.config/mugen/config.toml           # Daemon configuration
~/.config/mugen/apps/                 # Registered app manifests
~/.config/systemd/user/mugen.service  # Systemd user service definition
~/.local/share/mugen/launcher/        # Mugen Launcher AppImage
~/.local/share/mugen/apps/            # Installed app bundles
~/.local/share/mugen/profiles/        # Per-game profile data
~/.local/share/mugen/logs/            # Daemon and app logs
~/.local/share/mugen/cache/           # Trainer cache, metadata cache
```

---

## Appendix B — Mugen vs Decky Comparison

| Feature | Decky Loader | Mugen |
|---------|-------------|-------|
| Survives SteamOS updates | ❌ Breaks every major update | ✅ Lives in `~/.local/` |
| Quick Access Menu integration | ✅ Native (injects into QAM) | ✅ Via small Decky plugin doorbell |
| Cross-platform | ❌ Steam Deck only | ✅ Deck, Windows, iOS, Android, Web |
| Cloud sync | ❌ No | ✅ Yes (Mugen Pro, E2E encrypted) |
| Mobile companion | ❌ No | ✅ Yes (Phase 3) |
| Frame generation control | ⚠️ lsfg-vk plugin (no per-game profiles) | ✅ LosslessDeck (with per-game profiles) |
| Trainer management | ❌ CheatDeck (broken Nov 2025) | ✅ SharkDeck (maintained) |
| Performance tuning | ✅ PowerTools plugin | ✅ PowerMugen (built-in) |
| Update model | ❌ Manual reinstall after Valve updates | ✅ Auto-update via daemon |
| Monetization | Donations only | Free + Pro ($4/month) |

---

*Mugen — Vectrx — Confidential — March 2026*
