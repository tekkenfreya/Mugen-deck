use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio::time::{self, Duration};
use tracing::{debug, info, warn};

/// Poll interval when no game is running — low frequency to minimize overhead.
const IDLE_POLL_SECS: u64 = 30;
/// Poll interval when a game is running — faster for responsive cheat panel.
const ACTIVE_POLL_SECS: u64 = 5;

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

    /// Starts the background polling loop with adaptive intervals.
    ///
    /// Polls every 15s when idle, every 5s when a game is running.
    /// All blocking filesystem I/O is offloaded to `spawn_blocking` to
    /// avoid starving the tokio async runtime.
    pub fn start_polling(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                // Determine poll interval based on whether a game is running
                let has_game = self.current_game.read().await.is_some();
                let poll_secs = if has_game {
                    ACTIVE_POLL_SECS
                } else {
                    IDLE_POLL_SECS
                };

                time::sleep(Duration::from_secs(poll_secs)).await;

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
    ///
    /// Uses `spawn_blocking` to keep blocking I/O off the async runtime.
    async fn scan_library(&self) -> Result<()> {
        let steamapps = Self::steamapps_dir()?;
        let last_scan_ref = self.last_scan.clone();

        // Snapshot mtimes under read lock (cheap — just clone the HashMap)
        let last_scan_snapshot = self.last_scan.read().await.clone();

        // Offload all blocking filesystem work to a dedicated thread
        let (needs_rescan, new_scan_map, games) = tokio::task::spawn_blocking(move || {
            scan_library_blocking(&steamapps, &last_scan_snapshot)
        })
        .await
        .context("library scan task panicked")??;

        // Update scan timestamps
        if !new_scan_map.is_empty() {
            let mut last_scan = last_scan_ref.write().await;
            for (path, mtime) in new_scan_map {
                last_scan.insert(path, mtime);
            }
        }

        // Update library if changed
        if needs_rescan {
            info!(count = games.len(), "library scan complete");
            *self.library.write().await = games;
        }

        Ok(())
    }

    /// Detects a running Steam game.
    ///
    /// Fast path: if a game is already detected, just checks if that PID is still alive.
    /// Slow path: full `/proc` scan, offloaded to `spawn_blocking`.
    async fn detect_running_game(&self) -> Result<()> {
        // Fast path: check if the known game PID is still running
        if let Some(current) = self.current_game.read().await.clone() {
            let pid = current.pid;
            let still_running = tokio::task::spawn_blocking(move || is_pid_alive(pid))
                .await
                .context("pid check panicked")?;

            if still_running {
                return Ok(());
            }

            // Game exited — clear and fall through to full scan
            debug!(pid = pid, game = %current.name, "game process exited");
            *self.current_game.write().await = None;
        }

        // Slow path: full /proc scan — offloaded to blocking thread
        let library_ref = self.library.clone();
        let running = tokio::task::spawn_blocking(move || {
            let library = library_ref.blocking_read();
            if library.is_empty() {
                return Ok(None);
            }
            find_running_steam_game(&library)
        })
        .await
        .context("proc scan panicked")??;

        if let Some(ref game) = running {
            info!(app_id = %game.app_id, name = %game.name, pid = game.pid, "game detected");
        }

        *self.current_game.write().await = running;
        Ok(())
    }
}

/// App IDs that are Steam tools/runtimes, not actual games.
/// These must be excluded from game detection.
const IGNORED_APP_IDS: &[&str] = &[
    "228980",  // Steamworks Common Redistributables
    "1070560", // Steam Linux Runtime 1.0 (scout)
    "1391110", // Steam Linux Runtime 2.0 (soldier)
    "1628350", // Steam Linux Runtime 3.0 (sniper)
];

/// Returns true if this game entry is a Steam tool/runtime, not a real game.
fn is_steam_tool(game: &SteamGame) -> bool {
    if IGNORED_APP_IDS.contains(&game.app_id.as_str()) {
        return true;
    }
    let lower = game.name.to_lowercase();
    lower.contains("steam linux runtime")
        || lower.contains("proton ")
        || lower.contains("steamworks")
}

/// Blocking library scan — runs on a dedicated thread via `spawn_blocking`.
///
/// Returns (needs_rescan, updated_mtime_map, games).
fn scan_library_blocking(
    steamapps: &Path,
    last_scan: &HashMap<PathBuf, SystemTime>,
) -> Result<(bool, HashMap<PathBuf, SystemTime>, Vec<SteamGame>)> {
    if !steamapps.exists() {
        debug!(path = %steamapps.display(), "steamapps directory not found");
        return Ok((false, HashMap::new(), Vec::new()));
    }

    let mut needs_rescan = false;
    let mut new_scan_map = HashMap::new();

    let entries = std::fs::read_dir(steamapps)
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
                    new_scan_map.insert(path, modified);
                }
            }
        }
    }

    if !needs_rescan {
        return Ok((false, new_scan_map, Vec::new()));
    }

    let mut games = Vec::new();
    let entries = std::fs::read_dir(steamapps)?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("acf") {
            continue;
        }
        match parse_acf(&path) {
            Ok(game) => {
                if is_steam_tool(&game) {
                    debug!(app_id = %game.app_id, name = %game.name, "skipping steam tool");
                    continue;
                }
                games.push(game);
            }
            Err(e) => warn!(path = %path.display(), error = %e, "failed to parse .acf"),
        }
    }

    Ok((true, new_scan_map, games))
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

/// Checks if a process with the given PID is still alive.
///
/// Lightweight — just checks if `/proc/<pid>` exists.
fn is_pid_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

/// Scans `/proc` for running Steam game processes.
///
/// This is the expensive operation — reads cmdline for every process.
/// Must run on a blocking thread via `spawn_blocking`.
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

        // Skip low PIDs (kernel threads, init) — games are always high PIDs
        if pid < 1000 {
            continue;
        }

        let cmdline_path = entry.path().join("cmdline");
        let cmdline = match std::fs::read_to_string(&cmdline_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Skip short cmdlines and known non-game processes
        if cmdline.len() < 5
            || cmdline.contains("steamwebhelper")
            || cmdline.contains("steam-runtime")
        {
            continue;
        }

        // Look for game executables in the steam library paths
        for game in library {
            if !game.install_dir.is_empty() && cmdline.contains(&game.install_dir) {
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
