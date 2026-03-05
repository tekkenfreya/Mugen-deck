use anyhow::{Context, Result};
use reqwest::header::HeaderMap;
use scraper::{Html, Selector};
use tracing::{debug, info, warn};

use super::types::TrainerInfo;

const GCW_BASE: &str = "https://gamecopyworld.eu/games";
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";

/// Converts a game name into a GCW URL slug.
///
/// Rules: lowercase, spaces become underscores, drop non-alphanumeric/non-underscore,
/// collapse multiple underscores.
fn slugify(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    for ch in name.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
        } else if ch == ' ' || ch == '_' {
            slug.push('_');
        }
        // drop everything else (apostrophes, colons, etc.)
    }
    // collapse multiple underscores
    let mut result = String::with_capacity(slug.len());
    let mut prev_underscore = false;
    for ch in slug.chars() {
        if ch == '_' {
            if !prev_underscore {
                result.push('_');
            }
            prev_underscore = true;
        } else {
            result.push(ch);
            prev_underscore = false;
        }
    }
    // trim trailing underscore
    result.trim_end_matches('_').to_string()
}

/// Builds the game page URL from a slug.
fn game_page_url(slug: &str) -> String {
    format!("{}/pc_{}.shtml", GCW_BASE, slug)
}

/// Builds browser-like headers matching the Fling scraper pattern.
fn browser_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
            .parse()
            .unwrap(),
    );
    headers.insert("Accept-Language", "en-US,en;q=0.5".parse().unwrap());
    headers.insert("DNT", "1".parse().unwrap());
    headers.insert("Connection", "keep-alive".parse().unwrap());
    headers.insert("Upgrade-Insecure-Requests", "1".parse().unwrap());
    headers
}

/// Detects Cloudflare challenge / anti-bot pages.
fn is_cloudflare_challenge(html: &str) -> bool {
    html.contains("cf-challenge")
        || html.contains("Just a moment...")
        || html.contains("checking your browser")
}

/// Extracts the URL from a `cbox('...')` pattern in an `onmousedown` attribute.
///
/// GCW hides real download URLs inside `onmousedown="cbox('https://...')"` while
/// the visible `href` is a fallback `enable_javascript.shtml`.
fn extract_cbox_url(onmousedown: &str) -> Option<String> {
    let start = onmousedown.find("cbox('")?;
    let rest = &onmousedown[start + 6..]; // skip "cbox('"
    let end = rest.find("')")?;
    let url = &rest[..end];
    // Decode HTML entities that might appear in attribute values
    Some(url.replace("&amp;", "&"))
}

/// Extracts a version range like `v1.02 - v1.16.1` or a single version from a trainer title.
///
/// GCW trainer titles typically look like:
/// `ELDEN RING v1.02 - v1.16.1 +56 TRAINER`
fn extract_version(title: &str) -> Option<String> {
    // Look for patterns like "v1.02 - v1.16.1" or just "v1.02"
    let lower = title.to_lowercase();
    let v_pos = lower.find('v')?;
    let rest = &title[v_pos..];

    // Find the end of the first version number
    let first_end = rest[1..]
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .map(|e| e + 1)
        .unwrap_or(rest.len());

    let first_version = &rest[..first_end];
    // Must start with v followed by a digit
    if first_version.len() <= 1 || !first_version[1..].starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }

    // Check for a range like " - v1.16.1"
    let after_first = &rest[first_end..];
    let trimmed = after_first.trim_start();
    if trimmed.starts_with("- ") || trimmed.starts_with("- v") || trimmed.starts_with("-v") {
        let dash_rest = trimmed.trim_start_matches('-').trim_start();
        if dash_rest.starts_with('v') || dash_rest.starts_with('V') {
            let second_end = dash_rest[1..]
                .find(|c: char| !c.is_ascii_digit() && c != '.')
                .map(|e| e + 1)
                .unwrap_or(dash_rest.len());
            let second_version = &dash_rest[..second_end];
            if second_version.len() > 1 {
                return Some(format!("{} - {}", first_version, second_version));
            }
        }
    }

    Some(first_version.to_string())
}

/// Parses trainer entries from a GCW game page HTML.
///
/// GCW game pages list trainers in a pattern like:
/// ```html
/// <strong>GAME NAME v1.0 +10 TRAINER</strong>
/// ...
/// <a onmousedown="cbox('https://dl.gamecopyworld.com/...')" href="enable_javascript.shtml">
/// ```
fn parse_game_page(html: &str, game_name: &str) -> Vec<TrainerInfo> {
    let document = Html::parse_document(html);
    let mut trainers = Vec::new();

    // Find all <strong> tags — trainer titles are wrapped in <strong>
    let Ok(strong_sel) = Selector::parse("strong") else {
        return trainers;
    };
    let Ok(a_sel) = Selector::parse("a[onmousedown]") else {
        return trainers;
    };

    // Collect all cbox links from the page
    let cbox_links: Vec<String> = document
        .select(&a_sel)
        .filter_map(|el| {
            let attr = el.value().attr("onmousedown")?;
            extract_cbox_url(attr)
        })
        .collect();

    let mut cbox_idx = 0;

    for el in document.select(&strong_sel) {
        let text = el.text().collect::<String>();
        let text = text.trim();
        let upper = text.to_uppercase();

        if !upper.contains("TRAINER") {
            continue;
        }

        // This <strong> is a trainer title — pair it with the next cbox link
        let download_url = if cbox_idx < cbox_links.len() {
            let url = cbox_links[cbox_idx].clone();
            cbox_idx += 1;
            url
        } else {
            continue; // no download link to pair with
        };

        let version = extract_version(text).unwrap_or_else(|| "unknown".to_string());

        trainers.push(TrainerInfo {
            name: text.to_string(),
            game_name: game_name.to_lowercase(),
            version,
            download_url,
            file_size: None,
            checksum: None,
            source: "gcw".to_string(),
        });
    }

    trainers
}

/// Determines which GCW index page a game name falls under.
///
/// GCW organizes games into alphabetical index pages:
/// - `gc_a-e.shtml` for A-E
/// - `gc_f-m.shtml` for F-M
/// - `gc_n-s.shtml` for N-S
/// - `gc_t-z.shtml` for T-Z
fn index_page_for(game_name: &str) -> &'static str {
    let first = game_name
        .trim()
        .chars()
        .next()
        .unwrap_or('a')
        .to_ascii_lowercase();
    match first {
        'a'..='e' => "gc_a-e.shtml",
        'f'..='m' => "gc_f-m.shtml",
        'n'..='s' => "gc_n-s.shtml",
        _ => "gc_t-z.shtml",
    }
}

/// Searches for trainers matching the given game name on GameCopyWorld.
///
/// First tries a direct slug-based URL. On 404, falls back to searching
/// the alphabetical index page.
pub async fn search_trainers(
    client: &reqwest::Client,
    game_name: &str,
) -> Result<Vec<TrainerInfo>> {
    let slug = slugify(game_name);
    let url = game_page_url(&slug);
    debug!(url = %url, slug = %slug, "searching GCW trainers");

    let headers = browser_headers();
    let response = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .headers(headers.clone())
        .send()
        .await
        .context("failed to fetch GCW game page")?;

    let status = response.status();
    if status.as_u16() == 404 {
        debug!("GCW direct slug 404, trying index fallback");
        return search_via_index(client, game_name).await;
    }

    if !status.is_success() {
        anyhow::bail!(
            "GCW game page returned HTTP {} — site may be blocking requests",
            status.as_u16()
        );
    }

    let html = response
        .text()
        .await
        .context("failed to read GCW response body")?;

    if is_cloudflare_challenge(&html) {
        anyhow::bail!(
            "GCW site returned an anti-bot challenge page — trainers cannot be fetched right now"
        );
    }

    let trainers = parse_game_page(&html, game_name);

    if trainers.is_empty() {
        let preview: String = html.chars().take(500).collect();
        warn!(html_preview = %preview, "GCW page parsed 0 results — HTML may have changed");
        // Try index fallback even if page loaded but no trainers found
        return search_via_index(client, game_name).await;
    }

    debug!(count = trainers.len(), "GCW trainers found");

    Ok(trainers)
}

/// Searches the GCW alphabetical index page for a fuzzy match on the game name,
/// then fetches the matched game page.
async fn search_via_index(client: &reqwest::Client, game_name: &str) -> Result<Vec<TrainerInfo>> {
    let index_file = index_page_for(game_name);
    let index_url = format!("{}/{}", GCW_BASE, index_file);
    debug!(url = %index_url, "searching GCW index page");

    let headers = browser_headers();
    let response = client
        .get(&index_url)
        .header("User-Agent", USER_AGENT)
        .headers(headers)
        .send()
        .await
        .context("failed to fetch GCW index page")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "GCW index page returned HTTP {}",
            response.status().as_u16()
        );
    }

    let html = response
        .text()
        .await
        .context("failed to read GCW index page")?;

    if is_cloudflare_challenge(&html) {
        anyhow::bail!("GCW index returned an anti-bot challenge page");
    }

    // Parse index page for game links — they look like <a href="pc_game_name.shtml">Game Name</a>
    // The `Html` / `Selector` types from scraper are !Send, so all parsing must
    // complete inside a block that drops them before any `.await` points.
    let matched_hrefs: Vec<(String, String)> = {
        let document = Html::parse_document(&html);
        let Ok(a_sel) = Selector::parse("a[href]") else {
            return Ok(Vec::new());
        };

        let game_lower = game_name.to_lowercase();
        let game_words: Vec<&str> = game_lower.split_whitespace().collect();

        document
            .select(&a_sel)
            .filter_map(|el| {
                let href = el.value().attr("href")?;
                if !href.starts_with("pc_") || !href.ends_with(".shtml") {
                    return None;
                }
                let link_text = el.text().collect::<String>();
                let link_lower = link_text.trim().to_lowercase();
                let matches = game_words.iter().all(|w| link_lower.contains(w));
                if matches {
                    Some((href.to_string(), link_text.trim().to_string()))
                } else {
                    None
                }
            })
            .collect()
    };

    for (href, link_text) in &matched_hrefs {
        info!(matched = %link_text, href = %href, "GCW index match found");

        // Fetch the matched game page
        let page_url = format!("{}/{}", GCW_BASE, href);
        let headers2 = browser_headers();
        let page_resp = client
            .get(&page_url)
            .header("User-Agent", USER_AGENT)
            .headers(headers2)
            .send()
            .await
            .context("failed to fetch matched GCW game page")?;

        if !page_resp.status().is_success() {
            continue;
        }

        let page_html = page_resp
            .text()
            .await
            .context("failed to read matched page")?;
        let trainers = parse_game_page(&page_html, game_name);

        if !trainers.is_empty() {
            return Ok(trainers);
        }
    }

    Ok(Vec::new())
}

// ---------------------------------------------------------------------------
// Download URL resolution (3-step chain)
// ---------------------------------------------------------------------------

/// Extracts the first `cbox('...')` URL from an HTML page.
///
/// Used on intermediate download pages (dl.gamecopyworld.com) to find the mirror link.
fn extract_first_cbox_url(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let sel = Selector::parse("a[onmousedown]").ok()?;
    for el in document.select(&sel) {
        let attr = el.value().attr("onmousedown")?;
        if let Some(url) = extract_cbox_url(attr) {
            return Some(url);
        }
    }
    None
}

/// Finds the final download link pointing to `mobiletarget.net` in the HTML.
///
/// The consoletarget.com page contains a direct `<a href>` to the `.zip`/`.rar`
/// file on `mobiletarget.net`. The href may use protocol-relative `//` URLs.
fn parse_final_download_link(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let sel = Selector::parse("a[href]").ok()?;
    for el in document.select(&sel) {
        let href = el.value().attr("href")?;
        if href.contains("mobiletarget.net") {
            // Normalize protocol-relative URLs
            let url = if href.starts_with("//") {
                format!("https:{}", href)
            } else {
                href.to_string()
            };
            return Some(url);
        }
    }
    None
}

/// Resolves the 3-step GCW download chain to get the final download URL.
///
/// Chain: `dl.gamecopyworld.com` (file archive) → `g1.gamecopyworld.com` (mirror, 302)
///        → `consoletarget.com` → `mobiletarget.net` (final `.zip`/`.rar` URL)
///
/// # Arguments
/// * `client` — HTTP client (should be configured to follow redirects)
/// * `file_archive_url` — the initial `dl.gamecopyworld.com` URL from the game page
pub async fn resolve_download_url(
    client: &reqwest::Client,
    file_archive_url: &str,
) -> Result<String> {
    debug!(url = %file_archive_url, "resolving GCW download URL — step 1");

    // Step 1: Fetch dl.gamecopyworld.com page → extract mirror cbox URL
    let headers = browser_headers();
    let step1_resp = client
        .get(file_archive_url)
        .header("User-Agent", USER_AGENT)
        .headers(headers.clone())
        .send()
        .await
        .context("GCW download step 1: failed to fetch file archive page")?;

    if !step1_resp.status().is_success() {
        anyhow::bail!("GCW download step 1: HTTP {}", step1_resp.status().as_u16());
    }

    let step1_html = step1_resp
        .text()
        .await
        .context("GCW step 1: failed to read body")?;
    let mirror_url = extract_first_cbox_url(&step1_html)
        .context("GCW step 1: no mirror link found on file archive page")?;

    debug!(url = %mirror_url, "resolving GCW download URL — step 2");

    // Step 2: Follow mirror link (g1.gamecopyworld.com) — may 302 to consoletarget.com
    // reqwest follows redirects by default, so we land on consoletarget.com
    let step2_resp = client
        .get(&mirror_url)
        .header("User-Agent", USER_AGENT)
        .headers(headers.clone())
        .send()
        .await
        .context("GCW download step 2: failed to follow mirror link")?;

    if !step2_resp.status().is_success() {
        anyhow::bail!("GCW download step 2: HTTP {}", step2_resp.status().as_u16());
    }

    let step2_html = step2_resp
        .text()
        .await
        .context("GCW step 2: failed to read body")?;

    debug!("resolving GCW download URL — step 3: parsing final link");

    // Step 3: Parse consoletarget.com page for mobiletarget.net download link
    let final_url = parse_final_download_link(&step2_html)
        .context("GCW step 3: no mobiletarget.net link found on consoletarget page")?;

    info!(url = %final_url, "GCW download URL resolved");
    Ok(final_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Task 2: slugify + game_page_url tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_slugify_simple() {
        assert_eq!(slugify("Elden Ring"), "elden_ring");
    }

    #[test]
    fn test_slugify_numbers() {
        assert_eq!(slugify("Fallout 4"), "fallout_4");
    }

    #[test]
    fn test_slugify_apostrophes() {
        assert_eq!(slugify("Assassin's Creed"), "assassins_creed");
    }

    #[test]
    fn test_slugify_colons() {
        assert_eq!(slugify("Halo: Infinite"), "halo_infinite");
    }

    #[test]
    fn test_slugify_multiple_spaces() {
        assert_eq!(slugify("Dark   Souls   III"), "dark_souls_iii");
    }

    #[test]
    fn test_game_page_url() {
        assert_eq!(
            game_page_url("elden_ring"),
            "https://gamecopyworld.eu/games/pc_elden_ring.shtml"
        );
    }

    // -----------------------------------------------------------------------
    // Task 3: parse_game_page, extract_cbox_url, extract_version tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_cbox_url_basic() {
        let attr = "cbox('https://dl.gamecopyworld.com/dl/12345')";
        assert_eq!(
            extract_cbox_url(attr),
            Some("https://dl.gamecopyworld.com/dl/12345".to_string())
        );
    }

    #[test]
    fn test_extract_cbox_url_html_entities() {
        let attr = "cbox('https://dl.gamecopyworld.com/dl/file?id=1&amp;type=trainer')";
        assert_eq!(
            extract_cbox_url(attr),
            Some("https://dl.gamecopyworld.com/dl/file?id=1&type=trainer".to_string())
        );
    }

    #[test]
    fn test_extract_cbox_url_no_match() {
        assert_eq!(extract_cbox_url("onclick='doStuff()'"), None);
    }

    #[test]
    fn test_extract_version_range() {
        assert_eq!(
            extract_version("ELDEN RING v1.02 - v1.16.1 +56 TRAINER"),
            Some("v1.02 - v1.16.1".to_string())
        );
    }

    #[test]
    fn test_extract_version_single() {
        assert_eq!(
            extract_version("DARK SOULS III v1.15 +28 TRAINER"),
            Some("v1.15".to_string())
        );
    }

    #[test]
    fn test_extract_version_none() {
        assert_eq!(extract_version("SOME GAME TRAINER"), None);
    }

    #[test]
    fn test_parse_game_page_two_entries() {
        let html = r#"
        <html><body>
            <strong>ELDEN RING v1.02 - v1.10 +50 TRAINER</strong>
            <br/>
            <a onmousedown="cbox('https://dl.gamecopyworld.com/dl/t1')" href="enable_javascript.shtml">Download</a>

            <strong>ELDEN RING v1.12 +56 TRAINER</strong>
            <br/>
            <a onmousedown="cbox('https://dl.gamecopyworld.com/dl/t2')" href="enable_javascript.shtml">Download</a>
        </body></html>
        "#;

        let results = parse_game_page(html, "Elden Ring");
        assert_eq!(results.len(), 2);

        assert_eq!(results[0].name, "ELDEN RING v1.02 - v1.10 +50 TRAINER");
        assert_eq!(results[0].version, "v1.02 - v1.10");
        assert_eq!(
            results[0].download_url,
            "https://dl.gamecopyworld.com/dl/t1"
        );
        assert_eq!(results[0].source, "gcw");
        assert_eq!(results[0].game_name, "elden ring");

        assert_eq!(results[1].name, "ELDEN RING v1.12 +56 TRAINER");
        assert_eq!(results[1].version, "v1.12");
        assert_eq!(
            results[1].download_url,
            "https://dl.gamecopyworld.com/dl/t2"
        );
    }

    #[test]
    fn test_parse_game_page_empty() {
        let html = "<html><body><p>No trainers here</p></body></html>";
        let results = parse_game_page(html, "NonexistentGame");
        assert!(results.is_empty());
    }

    // -----------------------------------------------------------------------
    // Task 5: download resolution parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_first_cbox_url_from_page() {
        let html = r#"
        <html><body>
            <p>Select a mirror:</p>
            <a onmousedown="cbox('https://g1.gamecopyworld.com/dl/mirror1')" href="enable_javascript.shtml">Mirror 1</a>
            <a onmousedown="cbox('https://g2.gamecopyworld.com/dl/mirror2')" href="enable_javascript.shtml">Mirror 2</a>
        </body></html>
        "#;

        let url = extract_first_cbox_url(html);
        assert_eq!(
            url,
            Some("https://g1.gamecopyworld.com/dl/mirror1".to_string())
        );
    }

    #[test]
    fn test_extract_first_cbox_url_none() {
        let html = "<html><body><a href='https://example.com'>Link</a></body></html>";
        assert_eq!(extract_first_cbox_url(html), None);
    }

    #[test]
    fn test_parse_final_download_link_https() {
        let html = r#"
        <html><body>
            <a href="https://mobiletarget.net/files/trainer.zip">Download</a>
        </body></html>
        "#;

        assert_eq!(
            parse_final_download_link(html),
            Some("https://mobiletarget.net/files/trainer.zip".to_string())
        );
    }

    #[test]
    fn test_parse_final_download_link_protocol_relative() {
        let html = r#"
        <html><body>
            <a href="//mobiletarget.net/files/trainer.rar">Download</a>
        </body></html>
        "#;

        assert_eq!(
            parse_final_download_link(html),
            Some("https://mobiletarget.net/files/trainer.rar".to_string())
        );
    }

    #[test]
    fn test_parse_final_download_link_none() {
        let html = r#"
        <html><body>
            <a href="https://example.com/other">Other Link</a>
        </body></html>
        "#;

        assert_eq!(parse_final_download_link(html), None);
    }

    #[test]
    fn test_is_cloudflare_challenge() {
        assert!(is_cloudflare_challenge("blah cf-challenge blah"));
        assert!(is_cloudflare_challenge("Just a moment..."));
        assert!(!is_cloudflare_challenge("<html>Normal page</html>"));
    }

    #[test]
    fn test_index_page_for() {
        assert_eq!(index_page_for("Elden Ring"), "gc_a-e.shtml");
        assert_eq!(index_page_for("Fallout"), "gc_f-m.shtml");
        assert_eq!(index_page_for("Skyrim"), "gc_n-s.shtml");
        assert_eq!(index_page_for("The Witcher"), "gc_t-z.shtml");
    }
}
