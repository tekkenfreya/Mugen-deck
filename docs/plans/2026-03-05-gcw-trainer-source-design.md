# GameCopyWorld Trainer Source for SharkDeck

**Date:** 2026-03-05
**Status:** Approved

## Problem

SharkDeck currently only scrapes Fling Trainer (flingtrainer.com) for trainers. GameCopyWorld (GCW) has a much larger catalog including Fling trainers plus others. Adding GCW as a parallel source gives users more trainer options.

## Key Discovery: No Headless Browser Needed

GCW hides download URLs behind `href="enable_javascript.shtml"` fallbacks, but the real URLs are in `onmousedown` attributes:

```html
<a href="enable_javascript.shtml"
   onmousedown="cbox('https://dl.gamecopyworld.com/?c=19330&b=0&a=0&d=2017&f=...')">
```

This means `reqwest` + `scraper` (CSS selector parsing) can extract all download URLs — same stack already used for Fling.

## Architecture

### New Module

`daemon/src/sharkdeck/gcw.rs` — parallel to existing `fling.rs`. Both share the same `TrainerInfo` type and are called concurrently during search.

### Search Flow

1. User searches a game name in SharkDeck UI
2. Daemon calls Fling + GCW search in parallel via `tokio::join!`
3. GCW search: slugify game name → `pc_{slug}.shtml` → fetch page → parse trainer entries
4. Results merged (both sources), returned to frontend with `source` field distinguishing them

### GCW Game Page Scraping

**URL construction:** `https://gamecopyworld.eu/games/pc_{slug}.shtml`
**Slug rules:** lowercase, spaces → underscores, drop special chars

**Trainer entry structure in HTML:**
- `<strong>` — trainer title (game name, version range, +N cheat count)
- Text node — date (DD-MM-YYYY)
- Text node — author (FLiNG, MRANTIFUN, etc.)
- `<a onmousedown="cbox('...')">` — file archive URL
- Text node — `File Archive [X KB/MB]`

**Fallback for non-standard slugs:** If the constructed slug returns 404, fetch the relevant index page (`gcw_index.shtml` for A-E, `gcw_index_2.shtml` for F-M, etc.), fuzzy-match the game name, and use the correct slug.

### Download Flow (3-Step Chain)

All steps use plain HTTP — no JavaScript execution required.

```
Step 1: Game page (pc_*.shtml)
  Parse: onmousedown="cbox('https://dl.gamecopyworld.com/?c=XXXXX&b=0&a=0&d=YYYY&f=FILENAME')"

Step 2: dl.gamecopyworld.com page
  Parse: onmousedown for mirror link → g1.gamecopyworld.com/?y=...&x=...

Step 3: g1.gamecopyworld.com
  Follow: 302 redirect → d2.consoletarget.com page
  Parse: <a href="//g06.mobiletarget.net/?y=...&x=...">[ Mirror #1 ]</a>

Step 4: Download from mobiletarget.net → save .rar to cache
```

### dl.gamecopyworld.com URL Parameters

| Param | Example | Meaning |
|-------|---------|---------|
| `c` | `19330` | File/trainer numeric ID |
| `b` | `0` | Mirror index |
| `a` | `0` | Unknown flag |
| `d` | `2017` | Year |
| `f` | `FFXV+25Tr_LNG_v20220613!rar` | Filename (`.` → `!` for extension) |

### .rar Extraction

GCW serves trainers as `.rar` archives (unlike Fling which serves `.exe` directly). After download, the `.rar` must be extracted to get the trainer `.exe`.

## Changes Required

| File | Change |
|------|--------|
| `daemon/src/sharkdeck/gcw.rs` | **New** — GCW search, page parsing, 3-step download URL resolution |
| `daemon/src/sharkdeck/mod.rs` | Modify `search()` to call Fling + GCW in parallel, merge results |
| `daemon/src/sharkdeck/mod.rs` | Modify `enable_inner()` to handle GCW source (3-step download chain) |
| `daemon/src/sharkdeck/trainer.rs` | Add `.rar` extraction after download for GCW trainers |
| `daemon/src/sharkdeck/types.rs` | Ensure `source` field renders in UI |
| `daemon/Cargo.toml` | Add `unrar` crate for `.rar` extraction |

## gcw.rs Public API

```rust
/// Search GCW for trainers matching a game name.
/// Returns TrainerInfo items with source="gcw".
pub async fn search_trainers(
    client: &reqwest::Client,
    game_name: &str,
) -> anyhow::Result<Vec<TrainerInfo>>

/// Resolve a GCW trainer's download URL through the 3-step chain.
/// Returns the final mobiletarget.net direct download URL.
pub async fn resolve_download_url(
    client: &reqwest::Client,
    file_archive_url: &str,
) -> anyhow::Result<String>
```

## HTTP Details

- Reuse the same browser-like headers from `fling.rs` (User-Agent, Accept, etc.)
- Cloudflare detection: check for challenge page markers, return clean error
- Follow redirects for Step 3 (g1.gamecopyworld.com → consoletarget.com)
- reqwest redirect policy: allow cross-domain redirects for this chain

## Error Handling

- GCW unavailable / Cloudflare blocked → log warning, return empty results (Fling results still show)
- Slug miss (404) → try index page fallback → if still no match, return empty
- Any step in download chain fails → return error to SharkDeck manager, status set to Error
- .rar extraction fails → return error with filename context

## No Frontend Changes

The frontend already displays trainers from any source. The `source` field on `TrainerInfo` already exists. GCW trainers will appear alongside Fling trainers in the search results automatically.
