use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio::time::{self, Duration};
use tracing::{debug, info, warn};

/// Represents a Steam game detected from .acf manifest files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamGame {
    pub app_id: String,
    pub name: String,
    pub install_dir: String,
    pub size_on_disk: u64,
    pub state_flags: u32,
}

/// Represents the currently running game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningGame {
    pub app_id: String,
    pub name: String,
    pub pid: u32,
}

/// Shared game detection state.
#[derive(Debug, Clone)]
pub struct GameDetector {
    pub library: Arc<RwLock<Vec<SteamGame>>>,
    pub current_game: Arc<RwLock<Option<RunningGame>>>,
    last_scan: Arc<RwLock<HashMap<PathBuf, SystemTime>>>,
}

impl Default for GameDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl GameDetector {
    /// Creates a new game detector.
    pub fn new() -> Self {
        Self {
            library: Arc::new(RwLock::new(Vec::new())),
            current_game: Arc::new(RwLock::new(None)),
            last_scan: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns the Steam apps directory path.
    fn steamapps_dir() -> Result<PathBuf> {
        let home = std::env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("Steam")
            .join("steamapps"))
    }

    /// Starts the background polling loop (5s interval).
    pub fn start_polling(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                if let Err(e) = self.scan_library().await {
                    warn!(error = %e, "library scan failed");
                }
                if let Err(e) = self.detect_running_game().await {
                    debug!(error = %e, "running game detection failed");
                }
            }
        })
    }

    /// Scans `.acf` files to build the game library.
    async fn scan_library(&self) -> Result<()> {
        let steamapps = Self::steamapps_dir()?;
        if !steamapps.exists() {
            debug!(path = %steamapps.display(), "steamapps directory not found");
            return Ok(());
        }

        let mut needs_rescan = false;
        let mut last_scan = self.last_scan.write().await;

        let entries = std::fs::read_dir(&steamapps)
            .with_context(|| format!("failed to read {}", steamapps.display()))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("acf") {
                continue;
            }
            if let Ok(meta) = std::fs::metadata(&path) {
                if let Ok(modified) = meta.modified() {
                    let prev = last_scan.get(&path);
                    if prev.is_none() || prev.is_some_and(|t| *t != modified) {
                        needs_rescan = true;
                        last_scan.insert(path, modified);
                    }
                }
            }
        }

        if !needs_rescan {
            return Ok(());
        }

        let mut games = Vec::new();
        let entries = std::fs::read_dir(&steamapps)?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("acf") {
                continue;
            }
            match parse_acf(&path) {
                Ok(game) => games.push(game),
                Err(e) => warn!(path = %path.display(), error = %e, "failed to parse .acf"),
            }
        }

        info!(count = games.len(), "library scan complete");
        *self.library.write().await = games;
        Ok(())
    }

    /// Detects a running Steam game by scanning `/proc`.
    async fn detect_running_game(&self) -> Result<()> {
        let library = self.library.read().await;
        if library.is_empty() {
            return Ok(());
        }

        let running = find_running_steam_game(&library)?;
        *self.current_game.write().await = running;
        Ok(())
    }
}

/// Parses a Steam `.acf` manifest file into a `SteamGame`.
fn parse_acf(path: &Path) -> Result<SteamGame> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let app_id = extract_acf_value(&content, "appid").context("missing appid")?;
    let name = extract_acf_value(&content, "name").context("missing name")?;
    let install_dir = extract_acf_value(&content, "installdir").unwrap_or_default();
    let size_on_disk = extract_acf_value(&content, "SizeOnDisk")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let state_flags = extract_acf_value(&content, "StateFlags")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    Ok(SteamGame {
        app_id,
        name,
        install_dir,
        size_on_disk,
        state_flags,
    })
}

/// Extracts a quoted value for a given key from ACF/VDF content.
fn extract_acf_value(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        // ACF format: "key"		"value"
        if let Some(rest) = trimmed.strip_prefix(&format!("\"{}\"", key)) {
            let rest = rest.trim();
            if let Some(val) = rest.strip_prefix('"') {
                if let Some(end) = val.find('"') {
                    return Some(val[..end].to_string());
                }
            }
        }
    }
    None
}

/// Scans `/proc` for running Steam game processes.
fn find_running_steam_game(library: &[SteamGame]) -> Result<Option<RunningGame>> {
    let proc_dir = Path::new("/proc");
    if !proc_dir.exists() {
        return Ok(None);
    }

    let entries = std::fs::read_dir(proc_dir)?;

    for entry in entries.flatten() {
        let pid_str = entry.file_name();
        let pid_str = pid_str.to_string_lossy();
        let pid: u32 = match pid_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let cmdline_path = entry.path().join("cmdline");
        let cmdline = match std::fs::read_to_string(&cmdline_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Look for game executables in the steam library paths
        for game in library {
            if cmdline.contains(&game.install_dir) && !cmdline.contains("steamwebhelper") {
                return Ok(Some(RunningGame {
                    app_id: game.app_id.clone(),
                    name: game.name.clone(),
                    pid,
                }));
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_acf_value() {
        let content = r#"
"AppState"
{
	"appid"		"730"
	"name"		"Counter-Strike 2"
	"installdir"		"Counter-Strike Global Offensive"
	"SizeOnDisk"		"35000000000"
	"StateFlags"		"4"
}
"#;
        assert_eq!(extract_acf_value(content, "appid"), Some("730".to_string()));
        assert_eq!(
            extract_acf_value(content, "name"),
            Some("Counter-Strike 2".to_string())
        );
        assert_eq!(
            extract_acf_value(content, "installdir"),
            Some("Counter-Strike Global Offensive".to_string())
        );
    }

    #[test]
    fn missing_acf_value() {
        let content = r#""appid"		"730""#;
        assert_eq!(extract_acf_value(content, "name"), None);
    }
}
