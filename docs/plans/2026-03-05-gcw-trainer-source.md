# GameCopyWorld Trainer Source Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add GameCopyWorld as a parallel trainer source in SharkDeck, so search results show trainers from both Fling and GCW.

**Architecture:** New `gcw.rs` module alongside `fling.rs`. Search calls both in parallel via `tokio::join!`. GCW download uses a 3-step HTTP chain (game page → dl.gamecopyworld.com → consoletarget.com → mobiletarget.net). GCW serves `.rar` archives that need extraction.

**Tech Stack:** Rust, reqwest 0.12, scraper 0.21, unrar crate for .rar extraction, tokio for async

**Design doc:** `docs/plans/2026-03-05-gcw-trainer-source-design.md`

---

### Task 1: Add unrar dependency to Cargo.toml

**Files:**
- Modify: `daemon/Cargo.toml` (dependencies section, ~line 8-25)

**Step 1: Add the unrar crate**

Add `unrar = "0.5"` to `[dependencies]` in `daemon/Cargo.toml`, after the `tokio` line:

```toml
unrar = "0.5"
```

**Step 2: Verify it compiles**

Run: `cd daemon && cargo check 2>&1 | tail -5`
Expected: `Finished` with no errors

**Step 3: Commit**

```bash
git add daemon/Cargo.toml daemon/Cargo.lock
git commit -m "chore(daemon): add unrar crate for GCW .rar extraction"
```

---

### Task 2: Create gcw.rs — slug construction and constants

**Files:**
- Create: `daemon/src/sharkdeck/gcw.rs`
- Modify: `daemon/src/sharkdeck/mod.rs:1` (add `pub mod gcw;`)

**Step 1: Write tests for slug construction**

At the bottom of `gcw.rs`, add a test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_simple() {
        assert_eq!(slugify("Elden Ring"), "elden_ring");
    }

    #[test]
    fn test_slugify_numbers() {
        assert_eq!(slugify("Cyberpunk 2077"), "cyberpunk_2077");
    }

    #[test]
    fn test_slugify_special_chars() {
        assert_eq!(slugify("Assassin's Creed"), "assassins_creed");
    }

    #[test]
    fn test_slugify_colons() {
        assert_eq!(slugify("ELDEN RING: Shadow of the Erdtree"), "elden_ring_shadow_of_the_erdtree");
    }

    #[test]
    fn test_slugify_multiple_spaces() {
        assert_eq!(slugify("A  Hat  In  Time"), "a_hat_in_time");
    }

    #[test]
    fn test_game_page_url() {
        let url = game_page_url("elden_ring");
        assert_eq!(url, "https://gamecopyworld.eu/games/pc_elden_ring.shtml");
    }
}
```

**Step 2: Write the minimal implementation**

```rust
use anyhow::{bail, Context, Result};
use scraper::{Html, Selector};
use tracing::{debug, info, warn};

use super::types::TrainerInfo;

const GCW_BASE: &str = "https://gamecopyworld.eu/games";
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";

/// Convert a game name to a GCW URL slug.
/// "Elden Ring" → "elden_ring"
/// "Assassin's Creed" → "assassins_creed"
fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c == ' ' {
                '_'
            } else {
                // Drop apostrophes, colons, special chars
                '\0'
            }
        })
        .filter(|&c| c != '\0')
        .collect::<String>()
        // Collapse multiple underscores
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

/// Build the full game page URL from a slug.
fn game_page_url(slug: &str) -> String {
    format!("{}/pc_{}.shtml", GCW_BASE, slug)
}
```

**Step 3: Register the module**

Add to `daemon/src/sharkdeck/mod.rs` line 1 (after `pub mod fling;`):

```rust
pub mod gcw;
```

**Step 4: Run tests**

Run: `cd daemon && cargo test gcw -- --nocapture 2>&1 | tail -20`
Expected: All 6 tests pass

**Step 5: Commit**

```bash
git add daemon/src/sharkdeck/gcw.rs daemon/src/sharkdeck/mod.rs
git commit -m "feat(sharkdeck): add gcw module with slug construction"
```

---

### Task 3: GCW search — parse trainer entries from game page HTML

**Files:**
- Modify: `daemon/src/sharkdeck/gcw.rs`

**Step 1: Write test for parsing trainer entries from HTML**

Add to the test module:

```rust
    #[test]
    fn test_parse_trainer_entries() {
        let html = r#"
        <html><body>
        <strong>ELDEN RING v1.02 - v1.16.1 +35 TRAINER</strong>
        02-01-2026
        FLiNG
        <a href="enable_javascript.shtml" onmousedown="cbox('https://dl.gamecopyworld.com/?c=19330&amp;b=0&amp;a=0&amp;d=2026&amp;f=EldenRing+35Tr_FLiNG!rar' ); return false;"><img src="images/dsk.gif" border="0" height="25" alt="Click to Download!"></a>
        File Archive [927 KB] - EN/CN Text
        <strong>ELDEN RING v1.02 - v1.10 +28 TRAINER</strong>
        15-06-2024
        FLiNG
        <a href="enable_javascript.shtml" onmousedown="cbox('https://dl.gamecopyworld.com/?c=19331&amp;b=0&amp;a=0&amp;d=2024&amp;f=EldenRing+28Tr_FLiNG!rar' ); return false;"><img src="images/dsk.gif"></a>
        File Archive [800 KB]
        </body></html>
        "#;
        let results = parse_game_page(html, "Elden Ring");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "ELDEN RING v1.02 - v1.16.1 +35 TRAINER");
        assert!(results[0].download_url.contains("dl.gamecopyworld.com"));
        assert!(results[0].download_url.contains("c=19330"));
        assert_eq!(results[0].source, "gcw");
        assert_eq!(results[1].name, "ELDEN RING v1.02 - v1.10 +28 TRAINER");
    }

    #[test]
    fn test_extract_cbox_url() {
        let attr = "cbox('https://dl.gamecopyworld.com/?c=19330&amp;b=0&amp;a=0&amp;d=2026&amp;f=test!rar' ); return false;";
        let url = extract_cbox_url(attr).unwrap();
        assert_eq!(url, "https://dl.gamecopyworld.com/?c=19330&b=0&a=0&d=2026&f=test!rar");
    }

    #[test]
    fn test_parse_no_trainers() {
        let html = "<html><body><p>No trainers found</p></body></html>";
        let results = parse_game_page(html, "NonExistentGame");
        assert!(results.is_empty());
    }
```

**Step 2: Run tests to verify they fail**

Run: `cd daemon && cargo test gcw -- --nocapture 2>&1 | tail -10`
Expected: FAIL — `parse_game_page` and `extract_cbox_url` not defined

**Step 3: Implement parse_game_page and extract_cbox_url**

Add to `gcw.rs` above the test module:

```rust
/// Extract the URL from a cbox() onmousedown attribute.
/// Input: "cbox('https://dl.gamecopyworld.com/?c=19330&amp;b=0...' ); return false;"
/// Output: "https://dl.gamecopyworld.com/?c=19330&b=0..."
fn extract_cbox_url(onmousedown: &str) -> Option<String> {
    let start = onmousedown.find("cbox('")?;
    let url_start = start + 6; // len of "cbox('"
    let url_end = onmousedown[url_start..].find("'")?;
    let url = &onmousedown[url_start..url_start + url_end];
    // Decode HTML entities (&amp; → &)
    Some(url.replace("&amp;", "&"))
}

/// Parse trainer entries from a GCW game page HTML.
fn parse_game_page(html: &str, game_name: &str) -> Vec<TrainerInfo> {
    let document = Html::parse_document(html);
    let strong_sel = Selector::parse("strong").unwrap();
    let a_sel = Selector::parse("a[onmousedown]").unwrap();

    let mut trainers = Vec::new();

    // Collect all <strong> elements containing "TRAINER" (these are trainer titles)
    let titles: Vec<_> = document
        .select(&strong_sel)
        .filter(|el| {
            let text = el.text().collect::<String>();
            text.to_uppercase().contains("TRAINER")
        })
        .collect();

    // Collect all <a onmousedown="cbox(...)"> elements (download links)
    let links: Vec<_> = document
        .select(&a_sel)
        .filter_map(|el| {
            let attr = el.value().attr("onmousedown")?;
            if attr.contains("cbox(") {
                extract_cbox_url(attr)
            } else {
                None
            }
        })
        .collect();

    // Pair titles with download links (they appear in matching order)
    for (i, title_el) in titles.iter().enumerate() {
        let name = title_el.text().collect::<String>().trim().to_string();
        if name.is_empty() {
            continue;
        }

        let download_url = match links.get(i) {
            Some(url) => url.clone(),
            None => continue,
        };

        let version = extract_version(&name).unwrap_or_default();

        trainers.push(TrainerInfo {
            name: name.clone(),
            game_name: game_name.to_string(),
            version,
            download_url,
            file_size: None,
            checksum: None,
            source: "gcw".to_string(),
        });
    }

    trainers
}

/// Extract version string from a trainer title.
/// "ELDEN RING v1.02 - v1.16.1 +35 TRAINER" → "v1.02 - v1.16.1"
fn extract_version(title: &str) -> Option<String> {
    let lower = title.to_lowercase();
    let v_pos = lower.find('v')?;
    // Check char before v is not alphanumeric (avoid matching "reven" etc.)
    if v_pos > 0 && title.as_bytes()[v_pos - 1].is_ascii_alphanumeric() {
        return None;
    }
    let rest = &title[v_pos..];
    // Find the end: either "+N" or "TRAINER"
    let end = rest
        .find('+')
        .or_else(|| rest.to_uppercase().find("TRAINER"))
        .unwrap_or(rest.len());
    let version = rest[..end].trim().to_string();
    if version.len() > 1 {
        Some(version)
    } else {
        None
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cd daemon && cargo test gcw -- --nocapture 2>&1 | tail -15`
Expected: All tests pass

**Step 5: Commit**

```bash
git add daemon/src/sharkdeck/gcw.rs
git commit -m "feat(sharkdeck): parse GCW game page trainer entries"
```

---

### Task 4: GCW search — HTTP search function

**Files:**
- Modify: `daemon/src/sharkdeck/gcw.rs`

**Step 1: Implement the public search_trainers function**

Add above `parse_game_page`:

```rust
/// Build HTTP request headers matching a real browser.
fn browser_headers() -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".parse().unwrap());
    headers.insert("Accept-Language", "en-US,en;q=0.5".parse().unwrap());
    headers.insert("DNT", "1".parse().unwrap());
    headers.insert("Connection", "keep-alive".parse().unwrap());
    headers.insert("Upgrade-Insecure-Requests", "1".parse().unwrap());
    headers
}

/// Check if a response body looks like a Cloudflare challenge page.
fn is_cloudflare_challenge(html: &str) -> bool {
    html.contains("cf-challenge") || html.contains("Just a moment...")
        || html.contains("checking your browser")
}

/// Search GCW for trainers matching a game name.
/// Returns TrainerInfo items with source="gcw".
pub async fn search_trainers(
    client: &reqwest::Client,
    game_name: &str,
) -> Result<Vec<TrainerInfo>> {
    let slug = slugify(game_name);
    let url = game_page_url(&slug);
    debug!(url = %url, game = %game_name, "searching GCW");

    let resp = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .headers(browser_headers())
        .send()
        .await
        .context("GCW request failed")?;

    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        info!(slug = %slug, "GCW page not found, trying index fallback");
        return search_via_index(client, game_name).await;
    }
    if !status.is_success() {
        bail!("GCW returned HTTP {}", status);
    }

    let html = resp.text().await.context("reading GCW response body")?;

    if is_cloudflare_challenge(&html) {
        bail!("GCW blocked by Cloudflare challenge");
    }

    let trainers = parse_game_page(&html, game_name);
    info!(count = trainers.len(), game = %game_name, "GCW search results");
    Ok(trainers)
}

/// Fallback: search the GCW index pages for a game slug.
/// Tries to fuzzy-match the game name against index entries.
async fn search_via_index(
    client: &reqwest::Client,
    game_name: &str,
) -> Result<Vec<TrainerInfo>> {
    let first_char = game_name
        .chars()
        .find(|c| c.is_alphanumeric())
        .unwrap_or('a')
        .to_ascii_lowercase();

    let index_page = match first_char {
        'a'..='e' => "gcw_index.shtml",
        'f'..='m' => "gcw_index_2.shtml",
        'n'..='s' => "gcw_index_3.shtml",
        _ => "gcw_index_4.shtml",
    };

    let url = format!("{}/{}", GCW_BASE, index_page);
    debug!(url = %url, "fetching GCW index for fallback search");

    let resp = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .headers(browser_headers())
        .send()
        .await
        .context("GCW index request failed")?;

    if !resp.status().is_success() {
        bail!("GCW index returned HTTP {}", resp.status());
    }

    let html = resp.text().await?;
    if is_cloudflare_challenge(&html) {
        bail!("GCW index blocked by Cloudflare");
    }

    // Parse index page: find <a href="pc_*.shtml"> links
    let document = Html::parse_document(&html);
    let link_sel = Selector::parse("a[href]").unwrap();
    let game_lower = game_name.to_lowercase();

    for element in document.select(&link_sel) {
        let href = match element.value().attr("href") {
            Some(h) if h.starts_with("pc_") && h.ends_with(".shtml") => h,
            _ => continue,
        };
        let link_text = element.text().collect::<String>().to_lowercase();
        // Fuzzy match: check if the link text contains the game name words
        let words: Vec<&str> = game_lower.split_whitespace().collect();
        let matches = words.iter().filter(|w| link_text.contains(*w)).count();
        if matches >= words.len().saturating_sub(1) && matches > 0 {
            // Found a match — fetch that game page
            let slug = href.trim_start_matches("pc_").trim_end_matches(".shtml");
            let page_url = game_page_url(slug);
            debug!(page_url = %page_url, matched = %link_text, "index fallback match");

            let resp = client
                .get(&page_url)
                .header("User-Agent", USER_AGENT)
                .headers(browser_headers())
                .send()
                .await?;

            if resp.status().is_success() {
                let page_html = resp.text().await?;
                if !is_cloudflare_challenge(&page_html) {
                    let trainers = parse_game_page(&page_html, game_name);
                    if !trainers.is_empty() {
                        return Ok(trainers);
                    }
                }
            }
        }
    }

    debug!(game = %game_name, "no GCW index match found");
    Ok(Vec::new())
}
```

**Step 2: Verify it compiles**

Run: `cd daemon && cargo check 2>&1 | tail -5`
Expected: `Finished` with no errors

**Step 3: Commit**

```bash
git add daemon/src/sharkdeck/gcw.rs
git commit -m "feat(sharkdeck): GCW search with index fallback"
```

---

### Task 5: GCW download — 3-step URL resolution

**Files:**
- Modify: `daemon/src/sharkdeck/gcw.rs`

**Step 1: Write test for Mirror #1 link parsing**

Add to test module:

```rust
    #[test]
    fn test_parse_mirror_link() {
        let html = r#"
        <html><body>
        <a href="//g06.mobiletarget.net/?y=abc123&amp;x=encoded%2Bdata">[ Mirror #1 ]</a>
        <a href="https://www.7-zip.org/">7zip</a>
        </body></html>
        "#;
        let url = parse_final_download_link(html).unwrap();
        assert!(url.contains("mobiletarget.net"));
        assert!(url.starts_with("https://"));
    }

    #[test]
    fn test_parse_dl_page_mirror() {
        let html = r#"
        <html><body>
        <a href="enable_javascript.shtml" onmousedown="cbox('https://g1.gamecopyworld.com/?y=abc&amp;x=data' ); return false;">Mirror</a>
        </body></html>
        "#;
        let url = extract_first_cbox_url(html).unwrap();
        assert!(url.contains("g1.gamecopyworld.com"));
    }
```

**Step 2: Run tests to verify they fail**

Run: `cd daemon && cargo test gcw -- --nocapture 2>&1 | tail -10`
Expected: FAIL — `parse_final_download_link` and `extract_first_cbox_url` not defined

**Step 3: Implement the 3-step resolution**

Add to `gcw.rs`:

```rust
/// Extract the first cbox() URL from a page (used for dl.gamecopyworld.com pages).
fn extract_first_cbox_url(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let sel = Selector::parse("a[onmousedown]").unwrap();
    for el in document.select(&sel) {
        if let Some(attr) = el.value().attr("onmousedown") {
            if attr.contains("cbox(") {
                return extract_cbox_url(attr);
            }
        }
    }
    None
}

/// Parse the final download link (mobiletarget.net) from a consoletarget.com page.
fn parse_final_download_link(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let sel = Selector::parse("a[href]").unwrap();
    for el in document.select(&sel) {
        if let Some(href) = el.value().attr("href") {
            if href.contains("mobiletarget.net") {
                // Normalize protocol-relative URLs
                let url = if href.starts_with("//") {
                    format!("https:{}", href)
                } else {
                    href.to_string()
                };
                // Decode HTML entities
                return Some(url.replace("&amp;", "&"));
            }
        }
    }
    None
}

/// Resolve a GCW trainer download URL through the 3-step chain.
///
/// Step 1: file_archive_url (dl.gamecopyworld.com) → parse for mirror link
/// Step 2: mirror link (g1.gamecopyworld.com) → follow 302 → consoletarget.com
/// Step 3: consoletarget.com → parse for mobiletarget.net final download link
pub async fn resolve_download_url(
    client: &reqwest::Client,
    file_archive_url: &str,
) -> Result<String> {
    // Step 1: Fetch the dl.gamecopyworld.com file archive page
    debug!(url = %file_archive_url, "GCW resolve step 1: fetching file archive");
    let resp = client
        .get(file_archive_url)
        .header("User-Agent", USER_AGENT)
        .headers(browser_headers())
        .send()
        .await
        .context("GCW step 1: file archive request failed")?;

    if !resp.status().is_success() {
        bail!("GCW step 1: HTTP {}", resp.status());
    }
    let html = resp.text().await?;
    if is_cloudflare_challenge(&html) {
        bail!("GCW step 1: Cloudflare challenge");
    }

    let mirror_url = extract_first_cbox_url(&html)
        .context("GCW step 1: no mirror link found on file archive page")?;
    debug!(url = %mirror_url, "GCW resolve step 2: following mirror link");

    // Step 2: Follow the mirror link (g1.gamecopyworld.com → 302 → consoletarget.com)
    // Use a client that follows redirects to land on consoletarget.com
    let redirect_client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("building redirect client")?;

    let resp = redirect_client
        .get(&mirror_url)
        .headers(browser_headers())
        .send()
        .await
        .context("GCW step 2: mirror redirect failed")?;

    if !resp.status().is_success() {
        bail!("GCW step 2: HTTP {}", resp.status());
    }
    let html = resp.text().await?;

    // Step 3: Parse the consoletarget.com page for the mobiletarget.net link
    let download_url = parse_final_download_link(&html)
        .context("GCW step 3: no mobiletarget.net download link found")?;
    info!(url = %download_url, "GCW resolve complete: final download URL");

    Ok(download_url)
}
```

**Step 4: Run tests to verify they pass**

Run: `cd daemon && cargo test gcw -- --nocapture 2>&1 | tail -15`
Expected: All tests pass

**Step 5: Commit**

```bash
git add daemon/src/sharkdeck/gcw.rs
git commit -m "feat(sharkdeck): GCW 3-step download URL resolution"
```

---

### Task 6: Add .rar extraction support to trainer.rs

**Files:**
- Modify: `daemon/src/sharkdeck/trainer.rs`

**Step 1: Add extract_rar function**

Add to `trainer.rs`:

```rust
use std::path::Path;

/// Extract a .rar archive and return the path to the first .exe found inside.
/// Extracts to a subdirectory next to the .rar file.
pub async fn extract_rar(rar_path: &str) -> Result<String> {
    let rar = Path::new(rar_path);
    let extract_dir = rar.with_extension(""); // Remove .rar extension to make dir name

    // Create extraction directory
    tokio::fs::create_dir_all(&extract_dir)
        .await
        .context("creating extraction directory")?;

    let extract_dir_str = extract_dir.to_string_lossy().to_string();
    let rar_path_owned = rar_path.to_string();

    // unrar is blocking, run in spawn_blocking
    let exe_path = tokio::task::spawn_blocking(move || -> Result<String> {
        let archive = unrar::Archive::new(&rar_path_owned)
            .open_for_processing()
            .map_err(|e| anyhow::anyhow!("failed to open .rar: {}", e))?;

        let mut found_exe: Option<String> = None;

        let mut archive = archive;
        loop {
            match archive.read_header() {
                Ok(Some(header)) => {
                    let filename = header.entry().filename.to_string_lossy().to_string();
                    let is_exe = filename.to_lowercase().ends_with(".exe");
                    archive = header
                        .extract_to(&extract_dir_str)
                        .map_err(|e| anyhow::anyhow!("extract failed: {}", e))?;
                    if is_exe && found_exe.is_none() {
                        let full_path = Path::new(&extract_dir_str).join(&filename);
                        found_exe = Some(full_path.to_string_lossy().to_string());
                    }
                }
                Ok(None) => break,
                Err(e) => bail!("error reading .rar header: {}", e),
            }
        }

        found_exe.context("no .exe found in .rar archive")
    })
    .await
    .context("rar extraction task panicked")??;

    info!(exe = %exe_path, "extracted trainer from .rar");
    Ok(exe_path)
}
```

**Step 2: Modify download_trainer to handle .rar files**

In the existing `download_trainer` function, after the file is downloaded and checksum-verified, add a check for `.rar` extension. If the URL or filename ends in `.rar` (or `!rar` in GCW's encoding), extract and return the `.exe` path instead.

Add a new public wrapper function:

```rust
/// Download a trainer and extract if necessary (.rar archives).
/// Returns the path to the trainer .exe.
pub async fn download_and_extract_trainer(
    client: &reqwest::Client,
    trainer: &TrainerInfo,
    resolved_url: &str,
) -> Result<String> {
    let cached_path = download_trainer(client, trainer, resolved_url).await?;

    // Check if we downloaded a .rar file
    if cached_path.to_lowercase().ends_with(".rar")
        || resolved_url.contains("!rar")
        || resolved_url.contains(".rar")
    {
        debug!(path = %cached_path, "downloaded .rar, extracting");
        return extract_rar(&cached_path).await;
    }

    Ok(cached_path)
}
```

Also update the file extension logic in `download_trainer`. Currently it always saves as `.exe`. For GCW downloads, detect the extension from the URL. Modify the filename construction:

```rust
// Determine file extension from URL
let extension = if resolved_url.contains("!rar") || resolved_url.to_lowercase().contains(".rar") {
    "rar"
} else {
    "exe"
};
let filename = format!("{}.{}", safe_name, extension);
```

**Step 3: Add necessary imports**

Add at the top of `trainer.rs`:

```rust
use tracing::{debug, info};
```

**Step 4: Verify it compiles**

Run: `cd daemon && cargo check 2>&1 | tail -5`
Expected: `Finished` with no errors

**Step 5: Commit**

```bash
git add daemon/src/sharkdeck/trainer.rs
git commit -m "feat(sharkdeck): add .rar extraction for GCW trainers"
```

---

### Task 7: Integrate GCW into SharkDeckManager

**Files:**
- Modify: `daemon/src/sharkdeck/mod.rs`

**Step 1: Modify search() to call both sources in parallel**

Replace the `search` method body (lines ~79-106) with:

```rust
    pub async fn search(&self, game_name: &str) -> Result<SearchResult> {
        {
            let mut state = self.state.write().await;
            state.status = SharkDeckStatus::Searching;
            state.error = None;
        }
        let client = self.client().await;

        // Search both sources in parallel
        let (fling_result, gcw_result) = tokio::join!(
            fling::search_trainers(&client, game_name),
            gcw::search_trainers(&client, game_name),
        );

        let mut trainers = Vec::new();

        match fling_result {
            Ok(fling) => {
                debug!(count = fling.trainers.len(), "fling results");
                trainers.extend(fling.trainers);
            }
            Err(e) => {
                warn!(error = %e, "fling search failed");
            }
        }

        match gcw_result {
            Ok(gcw_trainers) => {
                debug!(count = gcw_trainers.len(), "gcw results");
                trainers.extend(gcw_trainers);
            }
            Err(e) => {
                warn!(error = %e, "gcw search failed");
            }
        }

        {
            let mut state = self.state.write().await;
            state.status = SharkDeckStatus::Idle;
        }
        Ok(SearchResult {
            query: game_name.to_string(),
            trainers,
            source: "fling+gcw".to_string(),
        })
    }
```

**Step 2: Modify enable_inner() to handle GCW source**

Replace the download section of `enable_inner` (lines ~146-154) with:

```rust
    async fn enable_inner(
        &self,
        trainer_info: &TrainerInfo,
        app_id: &str,
        game_pid: Option<u32>,
    ) -> Result<EnableResult> {
        {
            let mut state = self.state.write().await;
            state.status = SharkDeckStatus::Downloading;
        }
        let client = self.client().await;

        let trainer_path = if trainer_info.source == "gcw" {
            // GCW: 3-step download chain → .rar → extract .exe
            let download_url =
                gcw::resolve_download_url(&client, &trainer_info.download_url).await?;
            trainer::download_and_extract_trainer(&client, trainer_info, &download_url).await?
        } else {
            // Fling: resolve page → direct .exe download
            let resolved =
                fling::resolve_download_url(&client, &trainer_info.download_url).await?;
            trainer::download_trainer(&client, trainer_info, &resolved.download_url).await?
        };

        save_trainer_config(app_id, &trainer_path, trainer_info).await?;
        let launch_options = build_launch_options();
        {
            let mut state = self.state.write().await;
            state.status = SharkDeckStatus::Idle;
        }
        info!(app_id = %app_id, trainer = %trainer_path, "trainer enabled for game");
        Ok(EnableResult {
            trainer_path,
            launch_options,
            needs_restart: game_pid.is_some(),
        })
    }
```

**Step 3: Verify it compiles**

Run: `cd daemon && cargo check 2>&1 | tail -5`
Expected: `Finished` with no errors

**Step 4: Commit**

```bash
git add daemon/src/sharkdeck/mod.rs
git commit -m "feat(sharkdeck): integrate GCW as parallel trainer source"
```

---

### Task 8: Run full test suite and verify

**Files:** None (verification only)

**Step 1: Run all daemon tests**

Run: `cd daemon && cargo test 2>&1 | tail -20`
Expected: All tests pass

**Step 2: Run clippy**

Run: `cd daemon && cargo clippy -- -D warnings 2>&1 | tail -20`
Expected: No warnings

**Step 3: Check formatting**

Run: `cd daemon && cargo fmt --check 2>&1`
Expected: No formatting issues (or run `cargo fmt` to fix)

**Step 4: Final commit if any fixes needed**

```bash
git add -A daemon/
git commit -m "fix(sharkdeck): address clippy warnings and formatting"
```

---

## Summary of Files Changed

| File | Action | Purpose |
|------|--------|---------|
| `daemon/Cargo.toml` | Modify | Add `unrar` dependency |
| `daemon/src/sharkdeck/gcw.rs` | **Create** | GCW scraper — search, parse, 3-step download resolution |
| `daemon/src/sharkdeck/mod.rs` | Modify | Add `pub mod gcw`, parallel search, source-aware enable |
| `daemon/src/sharkdeck/trainer.rs` | Modify | Add `.rar` extraction, `download_and_extract_trainer()` |

## Test Coverage

- `test_slugify_simple` — basic name to slug
- `test_slugify_numbers` — numbers preserved
- `test_slugify_special_chars` — apostrophes dropped
- `test_slugify_colons` — colons dropped
- `test_slugify_multiple_spaces` — collapsed
- `test_game_page_url` — full URL construction
- `test_parse_trainer_entries` — HTML → TrainerInfo parsing
- `test_extract_cbox_url` — onmousedown attribute parsing
- `test_parse_no_trainers` — empty page handling
- `test_parse_mirror_link` — mobiletarget.net link extraction
- `test_parse_dl_page_mirror` — cbox URL from dl page
