use anyhow::{Context, Result};
use scraper::{Html, Selector};
use tracing::{debug, warn};

use super::types::{SearchResult, TrainerInfo};

/// A single cheat option scraped from a trainer page (informational only).
#[derive(Debug, Clone)]
pub struct CheatOption {
    pub hotkey: String,
    pub description: String,
}

const FLING_BASE: &str = "https://flingtrainer.com";
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";

/// Searches the Fling trainer database for trainers matching the given game name.
pub async fn search_trainers(client: &reqwest::Client, game_name: &str) -> Result<SearchResult> {
    let search_url = format!("{}/?s={}", FLING_BASE, urlencoding::encode(game_name));
    debug!(url = %search_url, "searching fling trainers");

    let response = client
        .get(&search_url)
        .header("User-Agent", USER_AGENT)
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header("Accept-Language", "en-US,en;q=0.5")
        .header("DNT", "1")
        .header("Connection", "keep-alive")
        .header("Upgrade-Insecure-Requests", "1")
        .send()
        .await
        .context("failed to fetch fling search results")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "fling search returned HTTP {} — site may be blocking requests",
            response.status().as_u16()
        );
    }

    let html = response
        .text()
        .await
        .context("failed to read response body")?;

    // Detect Cloudflare challenge / anti-bot pages
    if html.contains("cf-challenge")
        || html.contains("Just a moment...")
        || html.contains("checking your browser")
    {
        anyhow::bail!(
            "fling site returned an anti-bot challenge page — trainers cannot be fetched right now"
        );
    }

    let trainers = parse_search_results(&html, game_name);

    if trainers.is_empty() {
        // Log a snippet of the HTML to help diagnose parsing failures
        let preview: String = html.chars().take(500).collect();
        warn!(html_preview = %preview, "fling search parsed 0 results — HTML may have changed");
    } else {
        debug!(count = trainers.len(), "fling search results parsed");
    }

    Ok(SearchResult {
        query: game_name.to_string(),
        trainers,
        source: "flingtrainer.com".to_string(),
    })
}

/// Parses HTML search results from the Fling website.
///
/// Tries multiple selector strategies in case the site layout has changed.
fn parse_search_results(html: &str, game_name: &str) -> Vec<TrainerInfo> {
    let document = Html::parse_document(html);
    let game_lower = game_name.to_lowercase();

    // Strategy 1: article + .entry-title a / h2 a (original WordPress layout)
    let trainers = parse_with_selectors(&document, "article", ".entry-title a, h2 a", &game_lower);
    if !trainers.is_empty() {
        return trainers;
    }

    // Strategy 2: .post / .type-post containers (some WordPress themes)
    let trainers = parse_with_selectors(
        &document,
        ".post, .type-post",
        "a[href*=\"trainer\"], h2 a, .entry-title a",
        &game_lower,
    );
    if !trainers.is_empty() {
        return trainers;
    }

    // Strategy 3: li-based search results
    let trainers = parse_with_selectors(&document, "li", "a[href*=\"trainer\"]", &game_lower);
    if !trainers.is_empty() {
        return trainers;
    }

    // Strategy 4: broad fallback — any link containing "trainer" in the href
    if let Ok(sel) = Selector::parse("a[href*=\"trainer\"]") {
        let mut trainers = Vec::new();
        for el in document.select(&sel) {
            let title = el.text().collect::<String>().trim().to_string();
            let Some(href) = el.value().attr("href") else {
                continue;
            };
            if title.is_empty() || href.is_empty() {
                continue;
            }

            let lower_title = title.to_lowercase();
            if !lower_title.contains("trainer") && !lower_title.contains(&game_lower) {
                continue;
            }

            let version = extract_version(&title).unwrap_or_else(|| "unknown".to_string());
            trainers.push(TrainerInfo {
                name: title,
                game_name: game_lower.clone(),
                version,
                download_url: normalize_url(href),
                file_size: None,
                checksum: None,
                source: "flingtrainer.com".to_string(),
            });
        }
        if !trainers.is_empty() {
            return trainers;
        }
    }

    Vec::new()
}

/// Tries to parse search results using the given container and title selectors.
fn parse_with_selectors(
    document: &Html,
    container_sel: &str,
    title_sel: &str,
    game_lower: &str,
) -> Vec<TrainerInfo> {
    let Ok(container) = Selector::parse(container_sel) else {
        return Vec::new();
    };
    let Ok(title) = Selector::parse(title_sel) else {
        return Vec::new();
    };

    let mut trainers = Vec::new();

    for el in document.select(&container) {
        let Some(title_el) = el.select(&title).next() else {
            continue;
        };

        let name = title_el.text().collect::<String>().trim().to_string();
        let Some(href) = title_el.value().attr("href") else {
            continue;
        };
        if name.is_empty() || href.is_empty() {
            continue;
        }

        // Include if it mentions "trainer" OR matches the game name
        let lower_name = name.to_lowercase();
        if !lower_name.contains("trainer") && !lower_name.contains(game_lower) {
            continue;
        }

        let version = extract_version(&name).unwrap_or_else(|| "unknown".to_string());

        trainers.push(TrainerInfo {
            name,
            game_name: game_lower.to_string(),
            version,
            download_url: normalize_url(href),
            file_size: None,
            checksum: None,
            source: "flingtrainer.com".to_string(),
        });
    }

    trainers
}

/// Ensures a URL is absolute, prepending the Fling base if relative.
fn normalize_url(href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_string()
    } else if href.starts_with('/') {
        format!("{}{}", FLING_BASE, href)
    } else {
        format!("{}/{}", FLING_BASE, href)
    }
}

/// Extracts a version string like "v1.2.3" from a title.
fn extract_version(title: &str) -> Option<String> {
    let re_pattern = title
        .char_indices()
        .find(|(_, c)| *c == 'v' || *c == 'V')
        .and_then(|(i, _)| {
            let rest = &title[i..];
            // Match v followed by digits and dots
            let end = rest[1..]
                .find(|c: char| !c.is_ascii_digit() && c != '.')
                .map(|e| e + 1)
                .unwrap_or(rest.len());
            let candidate = &rest[..end];
            if candidate.len() > 1
                && candidate[1..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())
            {
                Some(candidate.to_string())
            } else {
                None
            }
        });
    re_pattern
}

/// Result of resolving a trainer detail page — includes download URL and scraped cheats.
#[derive(Debug, Clone)]
pub struct ResolvedTrainerPage {
    pub download_url: String,
    pub cheats: Vec<CheatOption>,
}

/// Fetches the actual download link and cheat hotkeys from a Fling trainer page.
///
/// The search results link to a page, not directly to the download.
pub async fn resolve_download_url(
    client: &reqwest::Client,
    trainer_page_url: &str,
) -> Result<ResolvedTrainerPage> {
    debug!(url = %trainer_page_url, "resolving download URL");

    let response = client
        .get(trainer_page_url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .context("failed to load trainer page")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "failed to load trainer page: {}",
            response.status().as_u16()
        );
    }

    let html = response
        .text()
        .await
        .context("failed to read trainer page")?;
    let document = Html::parse_document(&html);

    // Scrape cheat hotkeys from the page content
    let cheats = parse_hotkeys(&html);
    debug!(
        count = cheats.len(),
        "parsed cheat hotkeys from trainer page"
    );

    // Look for download links — common patterns on Fling's site
    let selectors = [
        "a[href*=\"download\"]",
        ".download-link a",
        "a.btn-download",
    ];

    for sel_str in &selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = document.select(&sel).next() {
                if let Some(href) = el.value().attr("href") {
                    if !href.is_empty() {
                        let url = if href.starts_with('/') {
                            format!("{}{}", FLING_BASE, href)
                        } else {
                            href.to_string()
                        };
                        return Ok(ResolvedTrainerPage {
                            download_url: url,
                            cheats,
                        });
                    }
                }
            }
        }
    }

    // Fallback: look for any link containing "Download" text
    if let Ok(a_sel) = Selector::parse("a") {
        for el in document.select(&a_sel) {
            let text = el.text().collect::<String>();
            if text.to_lowercase().contains("download") {
                if let Some(href) = el.value().attr("href") {
                    if !href.is_empty() && href != "#" {
                        let url = if href.starts_with('/') {
                            format!("{}{}", FLING_BASE, href)
                        } else {
                            href.to_string()
                        };
                        return Ok(ResolvedTrainerPage {
                            download_url: url,
                            cheats,
                        });
                    }
                }
            }
        }
    }

    anyhow::bail!("could not find download link on trainer page")
}

/// Parses cheat hotkeys from the trainer page HTML.
///
/// Fling trainer pages list hotkeys as text lines like:
/// - `Num 1 – God Mode`
/// - `Ctrl+Num 1 – Edit Runes`
/// - `F1 - Infinite Health`
///
/// We split on em-dash (–), en-dash (—), or spaced hyphen (` - `) to separate hotkey from description.
fn parse_hotkeys(html: &str) -> Vec<CheatOption> {
    let document = Html::parse_document(html);
    let mut cheats = Vec::new();

    // Collect all text content from the page body
    let body_sel = Selector::parse("body").unwrap_or_else(|_| Selector::parse("html").unwrap());
    let Some(body) = document.select(&body_sel).next() else {
        return cheats;
    };

    let full_text = body.text().collect::<String>();

    for line in full_text.lines() {
        let line = line.trim();
        if line.is_empty() || line.len() > 200 {
            continue;
        }

        // Try splitting on em-dash, en-dash, or spaced hyphen
        let parts: Option<(&str, &str)> = if line.contains(" \u{2013} ") {
            // en-dash –
            line.split_once(" \u{2013} ")
        } else if line.contains(" \u{2014} ") {
            // em-dash —
            line.split_once(" \u{2014} ")
        } else if line.contains(" - ") {
            line.split_once(" - ")
        } else {
            None
        };

        let Some((hotkey_raw, desc_raw)) = parts else {
            continue;
        };

        let hotkey = hotkey_raw.trim().to_string();
        let description = desc_raw.trim().to_string();

        // Validate: hotkey should look like a key name (contains known key tokens)
        if !looks_like_hotkey(&hotkey) {
            continue;
        }

        // Skip if description is empty or too long
        if description.is_empty() || description.len() > 100 {
            continue;
        }

        cheats.push(CheatOption {
            hotkey,
            description,
        });
    }

    cheats
}

/// Checks if a string looks like a keyboard hotkey.
fn looks_like_hotkey(s: &str) -> bool {
    let upper = s.to_uppercase();
    let key_tokens = [
        "NUM",
        "F1",
        "F2",
        "F3",
        "F4",
        "F5",
        "F6",
        "F7",
        "F8",
        "F9",
        "F10",
        "F11",
        "F12",
        "CTRL",
        "ALT",
        "SHIFT",
        "HOME",
        "END",
        "INSERT",
        "DELETE",
        "BACKSPACE",
        "TAB",
        "CAPS",
        "SPACE",
        "PAGE",
        "ENTER",
    ];
    key_tokens.iter().any(|token| upper.contains(token))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_search_results_with_articles() {
        let html = r#"
        <html><body>
        <article>
            <h2><a href="https://flingtrainer.com/trainer/elden-ring-trainer/">Elden Ring v1.12 Trainer</a></h2>
        </article>
        <article>
            <h2><a href="https://flingtrainer.com/trainer/elden-ring-2/">Elden Ring Deluxe v2.0 Trainer</a></h2>
        </article>
        <article>
            <h2><a href="https://flingtrainer.com/unrelated/">Some Unrelated Post</a></h2>
        </article>
        </body></html>
        "#;

        let results = parse_search_results(html, "Elden Ring");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "Elden Ring v1.12 Trainer");
        assert_eq!(results[0].version, "v1.12");
        assert_eq!(
            results[0].download_url,
            "https://flingtrainer.com/trainer/elden-ring-trainer/"
        );
        assert_eq!(results[0].source, "flingtrainer.com");
        assert_eq!(results[1].version, "v2.0");
    }

    #[test]
    fn test_parse_search_results_empty() {
        let html = "<html><body><p>No results</p></body></html>";
        let results = parse_search_results(html, "NonexistentGame");
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_search_results_with_entry_title_class() {
        let html = r#"
        <html><body>
        <article>
            <div class="entry-title"><a href="/trainer/test/">Test Game Trainer v3.1</a></div>
        </article>
        </body></html>
        "#;

        let results = parse_search_results(html, "Test Game");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].version, "v3.1");
    }

    #[test]
    fn test_extract_version() {
        assert_eq!(
            extract_version("Game v1.2.3 Trainer"),
            Some("v1.2.3".to_string())
        );
        assert_eq!(
            extract_version("Game V2.0 Trainer"),
            Some("V2.0".to_string())
        );
        assert_eq!(extract_version("Game Trainer"), None);
        assert_eq!(extract_version("Game v10 Plus"), Some("v10".to_string()));
    }

    #[test]
    fn test_parse_filters_non_trainer_results() {
        let html = r#"
        <html><body>
        <article>
            <h2><a href="/news/update/">Random News Article</a></h2>
        </article>
        <article>
            <h2><a href="/trainer/game/">Game Trainer v1.0</a></h2>
        </article>
        </body></html>
        "#;

        let results = parse_search_results(html, "Game");
        assert_eq!(results.len(), 1);
        assert!(results[0].name.contains("Trainer"));
    }

    #[test]
    fn test_parse_hotkeys_em_dash() {
        let html = r#"<html><body>
        <p>Num 1 – God Mode</p>
        <p>Num 2 – Infinite Ammo</p>
        <p>Ctrl+Num 3 – Max Gold</p>
        </body></html>"#;

        let cheats = parse_hotkeys(html);
        assert_eq!(cheats.len(), 3);
        assert_eq!(cheats[0].hotkey, "Num 1");
        assert_eq!(cheats[0].description, "God Mode");
        assert_eq!(cheats[1].hotkey, "Num 2");
        assert_eq!(cheats[1].description, "Infinite Ammo");
        assert_eq!(cheats[2].hotkey, "Ctrl+Num 3");
        assert_eq!(cheats[2].description, "Max Gold");
    }

    #[test]
    fn test_parse_hotkeys_spaced_hyphen() {
        let html = r#"<html><body>
        <p>F1 - Unlimited Health</p>
        <p>F2 - No Clip</p>
        </body></html>"#;

        let cheats = parse_hotkeys(html);
        assert_eq!(cheats.len(), 2);
        assert_eq!(cheats[0].hotkey, "F1");
        assert_eq!(cheats[0].description, "Unlimited Health");
    }

    #[test]
    fn test_parse_hotkeys_ignores_non_keys() {
        let html = r#"<html><body>
        <p>Some random text - not a hotkey</p>
        <p>Version 1.0 - Released today</p>
        <p>Num 5 – Super Speed</p>
        </body></html>"#;

        let cheats = parse_hotkeys(html);
        assert_eq!(cheats.len(), 1);
        assert_eq!(cheats[0].hotkey, "Num 5");
    }

    #[test]
    fn test_looks_like_hotkey() {
        assert!(looks_like_hotkey("Num 1"));
        assert!(looks_like_hotkey("Ctrl+Num 3"));
        assert!(looks_like_hotkey("F1"));
        assert!(looks_like_hotkey("Alt+F4"));
        assert!(looks_like_hotkey("Shift+Tab"));
        assert!(!looks_like_hotkey("Some random text"));
        assert!(!looks_like_hotkey("Version 1.0"));
    }
}
