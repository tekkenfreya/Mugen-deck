use anyhow::{Context, Result};
use reqwest::header::HeaderMap;
use scraper::{Html, Selector};
use tracing::{debug, info};

use super::types::TrainerInfo;

const GCW_BASE: &str = "https://gamecopyworld.com/games";
/// GCW splits its game index across 4 pages by letter range:
/// - gcw_index.shtml: A-E
/// - gcw_index_2.shtml: F-M
/// - gcw_index_3.shtml: N-S
/// - gcw_index_4.shtml: T-Z
const GCW_INDEX_PAGES: &[&str] = &[
    "https://gamecopyworld.com/games/gcw_index.shtml",
    "https://gamecopyworld.com/games/gcw_index_2.shtml",
    "https://gamecopyworld.com/games/gcw_index_3.shtml",
    "https://gamecopyworld.com/games/gcw_index_4.shtml",
];
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";

/// Converts a game name into a GCW URL slug.
///
/// Rules: lowercase, spaces become underscores, drop non-alphanumeric/non-underscore,
/// collapse multiple underscores.
#[cfg(test)]
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
#[cfg(test)]
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
///
/// Note: GCW uses `cbox('URL' )` with a space before `)`, so we match on the
/// closing single quote only, not on `')`.
fn extract_cbox_url(onmousedown: &str) -> Option<String> {
    let start = onmousedown.find("cbox('")?;
    let rest = &onmousedown[start + 6..]; // skip "cbox('"
    let end = rest.find('\'')?; // find closing quote
    let url = rest[..end].trim();
    if url.is_empty() {
        return None;
    }
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
/// GCW game pages use `<a name="GAME v1.0 +10 TRAINER">` for trainer titles
/// and `<a onMouseDown="cbox('...')">` for download links. Each trainer title
/// is followed by its download link in DOM order, so we use a state machine:
/// walk all `<a>` elements, set the current title when we see a `name` attribute
/// containing "TRAINER", then pair it with the next `cbox(...)` link.
fn parse_game_page(html: &str, game_name: &str) -> Vec<TrainerInfo> {
    let document = Html::parse_document(html);
    let mut trainers = Vec::new();

    let Ok(a_sel) = Selector::parse("a") else {
        return trainers;
    };

    // State machine: track the current trainer title, pair with next cbox link
    let mut current_title: Option<String> = None;

    for el in document.select(&a_sel) {
        // Check for trainer title anchor: <a name="GAME v1.0 +10 TRAINER">
        if let Some(name_attr) = el.value().attr("name") {
            if name_attr.to_uppercase().contains("TRAINER") {
                current_title = Some(name_attr.to_string());
                continue;
            }
        }

        // Check for download link: <a onMouseDown="cbox('...')">
        if let Some(onmousedown) = el.value().attr("onmousedown") {
            if let Some(ref title) = current_title {
                if let Some(url) = extract_cbox_url(onmousedown) {
                    let version =
                        extract_version(title).unwrap_or_else(|| "unknown".to_string());

                    trainers.push(TrainerInfo {
                        name: title.clone(),
                        game_name: game_name.to_lowercase(),
                        version,
                        download_url: url,
                        file_size: None,
                        checksum: None,
                        source: "gcw".to_string(),
                    });
                    // Reset — this title has been paired
                    current_title = None;
                }
            }
        }
    }

    trainers
}


/// Searches for trainers matching the given game name on GameCopyWorld.
///
/// Strategy:
/// 1. Fetch all 4 index pages in parallel (A-E, F-M, N-S, T-Z — ~16k games total)
/// 2. Substring-match the game name against index entries
/// 3. Fetch matched game pages in parallel and parse trainer entries
///
/// The index approach is more reliable than guessing URL slugs, because
/// GCW slug conventions are inconsistent and bad slugs 301-redirect to
/// unrelated game pages instead of returning 404.
pub async fn search_trainers(
    client: &reqwest::Client,
    game_name: &str,
) -> Result<Vec<TrainerInfo>> {
    debug!(game = %game_name, "searching GCW trainers via index");

    let headers = browser_headers();

    // Fetch all 4 index pages in parallel
    let index_futures: Vec<_> = GCW_INDEX_PAGES
        .iter()
        .map(|url| {
            client
                .get(*url)
                .header("User-Agent", USER_AGENT)
                .headers(headers.clone())
                .send()
        })
        .collect();

    let index_responses = futures::future::join_all(index_futures).await;

    // Collect all matching game hrefs from all index pages
    let game_lower = game_name.to_lowercase();
    let mut matched_hrefs: Vec<String> = Vec::new();

    for resp_result in index_responses {
        let resp = match resp_result {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                debug!("GCW index page returned HTTP {}", r.status().as_u16());
                continue;
            }
            Err(e) => {
                debug!(error = %e, "GCW index page fetch failed");
                continue;
            }
        };

        let index_html = match resp.text().await {
            Ok(html) => html,
            Err(e) => {
                debug!(error = %e, "failed to read GCW index page body");
                continue;
            }
        };

        if is_cloudflare_challenge(&index_html) {
            debug!("GCW index page returned anti-bot challenge, skipping");
            continue;
        }

        // scraper types are !Send — all parsing must complete in this block
        let page_matches: Vec<String> = {
            let document = Html::parse_document(&index_html);
            let Ok(a_sel) = Selector::parse("a[href]") else {
                continue;
            };

            let mut matches: Vec<String> = Vec::new();
            for el in document.select(&a_sel) {
                let Some(href) = el.value().attr("href") else {
                    continue;
                };
                if !href.starts_with("pc_") || !href.ends_with(".shtml") {
                    continue;
                }
                let link_text = el.text().collect::<String>();
                let link_lower = link_text.trim().to_lowercase();

                if link_lower.contains(&game_lower) {
                    matches.push(href.to_string());
                }
            }
            matches
        };

        matched_hrefs.extend(page_matches);
    }

    if matched_hrefs.is_empty() {
        debug!("no GCW index match for '{}'", game_name);
        return Ok(Vec::new());
    }

    info!(count = matched_hrefs.len(), "GCW index matches found for '{}'", game_name);

    // Fetch all matched game pages in parallel
    let mut fetch_futures = Vec::new();
    for href in &matched_hrefs {
        let url = format!("{}/{}", GCW_BASE, href);
        let fut = client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .headers(headers.clone())
            .send();
        fetch_futures.push(fut);
    }

    let responses = futures::future::join_all(fetch_futures).await;

    let mut trainers = Vec::new();
    for resp_result in responses {
        match resp_result {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(page_html) = resp.text().await {
                    if !is_cloudflare_challenge(&page_html) {
                        let page_trainers = parse_game_page(&page_html, game_name);
                        trainers.extend(page_trainers);
                    }
                }
            }
            Ok(resp) => {
                debug!("GCW game page returned HTTP {}", resp.status().as_u16());
            }
            Err(e) => {
                debug!(error = %e, "GCW game page fetch failed");
            }
        }
    }

    if trainers.is_empty() {
        debug!("GCW pages loaded but no trainers found for '{}'", game_name);
    } else {
        info!(count = trainers.len(), "GCW trainers found");
    }

    Ok(trainers)
}


// ---------------------------------------------------------------------------
// Download URL resolution (3-step chain)
// ---------------------------------------------------------------------------

/// Extracts the first mirror link from a dl.gamecopyworld.com download page.
///
/// The download page uses plain `<a href="https://g1.gamecopyworld.com/...">MIRROR #01</a>`
/// links (not `cbox()` onmousedown). We look for `<a href>` pointing to
/// `g[N].gamecopyworld.com` domains, excluding image/status URLs like `online.gif`.
fn extract_mirror_url(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let sel = Selector::parse("a[href]").ok()?;
    for el in document.select(&sel) {
        let href = el.value().attr("href")?;
        // Match mirror links: https://g1.gamecopyworld.com/..., https://g4.gamecopyworld.com/...
        if href.contains("gamecopyworld.com")
            && !href.contains("dl.gamecopyworld.com")
            && !href.contains(".gif")
            && !href.contains(".png")
        {
            // Check it's a g[N].gamecopyworld.com mirror link (not a regular page link)
            if let Some(host_start) = href.find("://") {
                let after_proto = &href[host_start + 3..];
                if after_proto.starts_with('g')
                    && after_proto[1..]
                        .chars()
                        .next()
                        .map(|c| c.is_ascii_digit())
                        .unwrap_or(false)
                {
                    return Some(href.replace("&amp;", "&").to_string());
                }
            }
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
/// Retries up to 3 times with increasing delays if Cloudflare blocks the request.
/// Uses a cookie jar so Cloudflare cookies persist across the redirect chain.
pub async fn resolve_download_url(
    client: &reqwest::Client,
    file_archive_url: &str,
) -> Result<String> {
    let mut last_error = None;

    for attempt in 1..=3 {
        if attempt > 1 {
            let delay = std::time::Duration::from_secs(attempt * 2);
            info!(attempt, delay_secs = delay.as_secs(), "retrying GCW download after delay");
            tokio::time::sleep(delay).await;
        }

        match resolve_download_url_inner(client, file_archive_url).await {
            Ok(url) => return Ok(url),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("Cloudflare") || err_str.contains("anti-bot") {
                    info!(attempt, error = %e, "GCW blocked by Cloudflare, will retry");
                    last_error = Some(e);
                } else {
                    // Non-Cloudflare error — don't retry
                    return Err(e);
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("GCW download failed after 3 attempts")))
}

/// Single attempt at resolving the GCW download chain.
async fn resolve_download_url_inner(
    _client: &reqwest::Client,
    file_archive_url: &str,
) -> Result<String> {
    debug!(url = %file_archive_url, "resolving GCW download URL — step 1");

    let headers = browser_headers();
    let step1_url = if file_archive_url.contains("&nf=1") || file_archive_url.contains("?nf=1") {
        file_archive_url.to_string()
    } else {
        let sep = if file_archive_url.contains('?') { "&" } else { "?" };
        format!("{}{}nf=1", file_archive_url, sep)
    };

    // Use a cookie jar client so Cloudflare cookies persist across the chain.
    let cookie_jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
    let no_redirect_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_provider(cookie_jar.clone())
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default();

    let redirect_client = reqwest::Client::builder()
        .cookie_provider(cookie_jar)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default();

    // Step 1: Fetch dl.gamecopyworld.com page → extract mirror URL.
    let step1_resp = no_redirect_client
        .get(&step1_url)
        .header("User-Agent", USER_AGENT)
        .headers(headers.clone())
        .send()
        .await
        .context("GCW download step 1: failed to fetch file archive page")?;

    let step1_html = step1_resp
        .text()
        .await
        .context("GCW step 1: failed to read body")?;

    if is_cloudflare_challenge(&step1_html) {
        anyhow::bail!("GCW step 1: blocked by Cloudflare anti-bot challenge");
    }

    let mirror_url = extract_mirror_url(&step1_html)
        .context("GCW step 1: no mirror link found on file archive page")?;

    debug!(url = %mirror_url, "resolving GCW download URL — step 2");

    // Step 2: Follow mirror link → consoletarget.com (using cookie jar client)
    let step2_resp = redirect_client
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

    if is_cloudflare_challenge(&step2_html) {
        anyhow::bail!("GCW step 2: blocked by Cloudflare anti-bot challenge");
    }

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
            "https://gamecopyworld.com/games/pc_elden_ring.shtml"
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
        // Matches real GCW HTML structure: <a name="...TRAINER"> for titles,
        // <a onmousedown="cbox(...)"> for download links
        let html = r#"
        <html><body>
        <table>
            <tr>
                <td rowspan="2"><a name="ELDEN RING v1.02 - v1.10 +50 TRAINER">ELDEN RING v1.02 - v1.10 +50 TRAINER</a></td>
                <td>09-05-2024</td>
            </tr>
            <tr>
                <td><a href='enable_javascript.shtml' onMouseDown="cbox('https://dl.gamecopyworld.com/dl/t1'); return false;"><img src="images/dsk.gif"></a></td>
            </tr>

            <tr>
                <td rowspan="2"><a name="ELDEN RING v1.12 +56 TRAINER">ELDEN RING v1.12 +56 TRAINER</a></td>
                <td>01-01-2025</td>
            </tr>
            <tr>
                <td><a href='enable_javascript.shtml' onMouseDown="cbox('https://dl.gamecopyworld.com/dl/t2'); return false;"><img src="images/dsk.gif"></a></td>
            </tr>
        </table>
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
    fn test_parse_game_page_skips_non_trainer_sections() {
        // Non-trainer sections (FIXED FILES, NO-CD) should not be picked up
        let html = r#"
        <html><body>
        <table>
            <tr><td><a name="GAME v1.0 [M14] Fixed Files">Fixed Files</a></td></tr>
            <tr><td><a href='enable_javascript.shtml' onMouseDown="cbox('https://dl.gamecopyworld.com/dl/fix1'); return false;">DL</a></td></tr>

            <tr><td><a name="GAME v1.0 +10 TRAINER">GAME v1.0 +10 TRAINER</a></td></tr>
            <tr><td><a href='enable_javascript.shtml' onMouseDown="cbox('https://dl.gamecopyworld.com/dl/trainer1'); return false;">DL</a></td></tr>
        </table>
        </body></html>
        "#;

        let results = parse_game_page(html, "Game");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "GAME v1.0 +10 TRAINER");
        assert_eq!(
            results[0].download_url,
            "https://dl.gamecopyworld.com/dl/trainer1"
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
    fn test_extract_mirror_url_from_dl_page() {
        // Real HTML structure from dl.gamecopyworld.com download page
        let html = r#"
        <html><body>
            <p>Select a mirror:</p>
            <a href="https://g1.gamecopyworld.com/?y=25acfb3f&amp;x=tiC2HKypSx6" rel="nofollow">MIRROR #01</a>
            <a href="https://g2.gamecopyworld.com/?y=25acfb3f&amp;x=tiC2HKypSx6" rel="nofollow">MIRROR #02</a>
        </body></html>
        "#;

        let url = extract_mirror_url(html);
        assert_eq!(
            url,
            Some("https://g1.gamecopyworld.com/?y=25acfb3f&x=tiC2HKypSx6".to_string())
        );
    }

    #[test]
    fn test_extract_mirror_url_none() {
        let html = "<html><body><a href='https://example.com'>Link</a></body></html>";
        assert_eq!(extract_mirror_url(html), None);
    }

    #[test]
    fn test_extract_mirror_url_ignores_dl_domain() {
        // Should NOT match dl.gamecopyworld.com links (those are the page itself, not mirrors)
        let html = r#"
        <html><body>
            <a href="https://dl.gamecopyworld.com/some/page">Not a mirror</a>
            <a href="https://g1.gamecopyworld.com/?y=abc123">MIRROR #01</a>
        </body></html>
        "#;

        let url = extract_mirror_url(html);
        assert_eq!(
            url,
            Some("https://g1.gamecopyworld.com/?y=abc123".to_string())
        );
    }

    #[test]
    fn test_extract_mirror_url_ignores_online_gif() {
        // online.gif status images should not be matched as mirror links
        let html = r#"
        <html><body>
            <a href="https://g1.gamecopyworld.com/online.gif?nocache=12345"><img src="foo"></a>
            <a href="https://g1.gamecopyworld.com/?y=abc123&amp;x=xyz789" rel="nofollow">MIRROR #01</a>
        </body></html>
        "#;

        let url = extract_mirror_url(html);
        assert_eq!(
            url,
            Some("https://g1.gamecopyworld.com/?y=abc123&x=xyz789".to_string())
        );
    }

    #[test]
    fn test_extract_mirror_url_real_dl_page() {
        // Real HTML structure from dl.gamecopyworld.com (with &nf=1)
        let html = r#"
        <html><body>
        <table class="t1" width="100%" height="222" cellpadding="4"><tr><td align="center" valign="top"><b><font face="Arial" size="3">
        <img src="h1.gif" height="12" border="0"><br>
        <a href="https://g1.gamecopyworld.com/?y=086d2215&amp;x=379pLyP0iYyq6%2FC1Y1oPMozD" rel="nofollow">MIRROR #01</a>
        <img border="0" width="45" height="13" src="https://g1.gamecopyworld.com/online.gif?nocache=48384399"><br>
        <img src="h1.gif" height="12" border="0"><br>
        <a href="https://g4.gamecopyworld.com/?y=086d2215&amp;x=379pLyP0iYyq6%2FC1Y1oPMozD" rel="nofollow">MIRROR #02</a>
        </b></font></td></tr></table>
        </body></html>
        "#;

        let url = extract_mirror_url(html);
        assert_eq!(
            url,
            Some("https://g1.gamecopyworld.com/?y=086d2215&x=379pLyP0iYyq6%2FC1Y1oPMozD".to_string())
        );
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
    fn test_parse_real_gcw_html_structure() {
        // Exact HTML structure from a real GCW page (copy-pasted from curl output)
        let html = r#"
        <!DOCTYPE HTML PUBLIC "-//W3C//DTD HTML 4.01 Transitional//EN">
        <html><head>
        <script type="text/javascript">function cbox(url){jQuery.colorbox({width:"796",height:"670",iframe:true,href:url});}</script>
        </head><body>
        <table>
        <tr>
        <td rowspan="2"><a name="ELDEN RING: Shadow of the Erdtree v1.02 - v1.16.1 +35 TRAINER">ELDEN RING: Shadow of the Erdtree v1.02 - v1.16.1 +35 TRAINER</a></td>
        <td align="center" width="120"><font size="1">02-01-2026</font></td>
        </tr>
        <tr>
        <td width="28" align="center"><a href='enable_javascript.shtml' onMouseDown="cbox('https://dl.gamecopyworld.com/?c=19330&amp;b=0&amp;a=0&amp;d=2026&amp;f=Elden.Ring.Shadow.of.the.Erdtree.v1.02-v1.16.1.Plus.35.Trainer-FLiNG!rar' ); return false;"><img src="images/dsk.gif" border="0" height="25" alt="Click to Download!"></a></td>
        <td width="1"></td>
        <td>File Archive [927 KB]<span style="font-weight: 400"> - EN/CN Text</span></td>
        </tr>
        <tr>
        <td rowspan="2"><a name="ELDEN RING v1.02 - v1.10 +35 TRAINER">ELDEN RING v1.02 - v1.10 +35 TRAINER</a></td>
        <td align="center" width="120"><font size="1">09-05-2024</font></td>
        </tr>
        <tr>
        <td width="28" align="center"><a href='enable_javascript.shtml' onMouseDown="cbox('https://dl.gamecopyworld.com/?c=19330&amp;b=0&amp;a=0&amp;d=2024&amp;f=Elden.Ring.v1.02-v1.10.Plus.35.Trainer-FLiNG!rar' ); return false;"><img src="images/dsk.gif" border="0" height="25" alt="Click to Download!"></a></td>
        </tr>
        </table>
        </body></html>
        "#;

        let results = parse_game_page(html, "Elden Ring");
        println!("Results: {:?}", results.len());
        for r in &results {
            println!("  name={} url={}", r.name, r.download_url);
        }
        assert_eq!(results.len(), 2, "expected 2 trainers from real GCW HTML");
        assert!(results[0].name.contains("Shadow of the Erdtree"));
        assert!(results[0].download_url.contains("dl.gamecopyworld.com"));
        assert!(results[1].name.contains("v1.02 - v1.10"));
    }

    #[test]
    fn test_is_cloudflare_challenge() {
        assert!(is_cloudflare_challenge("blah cf-challenge blah"));
        assert!(is_cloudflare_challenge("Just a moment..."));
        assert!(!is_cloudflare_challenge("<html>Normal page</html>"));
    }

}
