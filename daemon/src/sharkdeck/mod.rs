pub mod fling;
pub mod gcw;
pub mod proton;
pub mod trainer;
pub mod types;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::RwLock;
use tracing::{info, warn};

use types::{
    EnableResult, SearchResult, SharkDeckStatus, SharkDeckStatusInfo, TrainerConfig, TrainerInfo,
    TrainerSummary,
};

/// Internal mutable state for the SharkDeck subsystem.
struct SharkDeckState {
    status: SharkDeckStatus,
    current_trainer: Option<TrainerInfo>,
    error: Option<String>,
    progress: Option<String>,
}

/// Manages the SharkDeck trainer lifecycle.
///
/// Trainers are launched via Proton's `PROTON_REMOTE_DEBUG_CMD` mechanism,
/// not by the daemon directly. The daemon handles:
/// - Searching for trainers (Fling database)
/// - Downloading trainer executables
/// - Installing .NET dependencies (winetricks)
/// - Writing per-game config files read by `trainer-hook.sh`
///
/// The hook script (`~/.local/share/sharkdeck/trainer-hook.sh`) runs before
/// each game launch and sets `PROTON_REMOTE_DEBUG_CMD` if a trainer is
/// configured for that game.
#[derive(Clone)]
pub struct SharkDeckManager {
    state: Arc<RwLock<SharkDeckState>>,
    http_client: Arc<RwLock<Option<reqwest::Client>>>,
}

impl Default for SharkDeckManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SharkDeckManager {
    /// Creates a new SharkDeckManager.
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(SharkDeckState {
                status: SharkDeckStatus::Idle,
                current_trainer: None,
                error: None,
                progress: None,
            })),
            http_client: Arc::new(RwLock::new(None)),
        }
    }

    /// Updates the progress message shown to the user.
    async fn set_progress(&self, msg: &str) {
        let mut state = self.state.write().await;
        state.progress = Some(msg.to_string());
    }

    /// Returns the HTTP client, creating it on first use.
    async fn client(&self) -> reqwest::Client {
        {
            let guard = self.http_client.read().await;
            if let Some(ref client) = *guard {
                return client.clone();
            }
        }
        let client = reqwest::Client::builder()
            .user_agent("sharkdeck-daemon")
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap_or_default();
        *self.http_client.write().await = Some(client.clone());
        client
    }

    /// Searches for trainers matching the given game name.
    ///
    /// Queries both Fling and GameCopyWorld in parallel, merging results.
    pub async fn search(&self, game_name: &str) -> Result<SearchResult> {
        {
            let mut state = self.state.write().await;
            state.status = SharkDeckStatus::Searching;
            state.error = None;
            state.progress = Some("Scanning trainer databases...".to_string());
        }

        let client = self.client().await;

        // Search both sources in parallel
        let (fling_result, gcw_result) = tokio::join!(
            fling::search_trainers(&client, game_name),
            gcw::search_trainers(&client, game_name),
        );

        let mut trainers = Vec::new();
        let mut sources = Vec::new();

        // Merge Fling results
        match fling_result {
            Ok(fling) => {
                info!(count = fling.trainers.len(), "fling results");
                trainers.extend(fling.trainers);
                sources.push("fling");
            }
            Err(e) => {
                warn!(error = %e, "fling search failed, continuing with other sources");
            }
        }

        // Merge GCW results
        match gcw_result {
            Ok(gcw_trainers) => {
                info!(count = gcw_trainers.len(), "gcw results");
                trainers.extend(gcw_trainers);
                sources.push("gcw");
            }
            Err(e) => {
                warn!(error = %e, "gcw search failed, continuing with other sources");
            }
        }

        let source = if sources.is_empty() {
            "none".to_string()
        } else {
            sources.join("+")
        };

        info!(
            total = trainers.len(),
            source = %source,
            query = %game_name,
            "combined trainer search complete"
        );

        {
            let mut state = self.state.write().await;
            state.status = SharkDeckStatus::Idle;
            state.progress = None;
        }

        Ok(SearchResult {
            query: game_name.to_string(),
            trainers,
            source,
        })
    }

    /// Starts the trainer enable process in a background task.
    ///
    /// Returns immediately — the frontend polls `/sharkdeck/status` for progress.
    /// This prevents the HTTP request from blocking for 30+ seconds during download,
    /// which caused Chrome tab suspension to silently kill the request.
    pub async fn start_enable(
        &self,
        trainer_info: TrainerInfo,
        app_id: String,
        game_pid: Option<u32>,
    ) {
        // Set status BEFORE spawning so the first status poll sees "downloading"
        {
            let mut state = self.state.write().await;
            state.status = SharkDeckStatus::Downloading;
            state.current_trainer = Some(trainer_info.clone());
            state.error = None;
            state.progress = Some("Preparing download...".to_string());
        }

        let this = self.clone();
        tokio::spawn(async move {
            match this.enable_inner(&trainer_info, &app_id, game_pid).await {
                Ok(_) => {
                    info!(app_id = %app_id, "trainer enabled successfully");
                }
                Err(e) => {
                    let mut state = this.state.write().await;
                    state.status = SharkDeckStatus::Idle;
                    state.error = Some(e.to_string());
                    state.progress = None;
                    tracing::warn!(error = %e, app_id = %app_id, "trainer enable failed");
                }
            }
        });
    }

    /// Inner enable logic — separated so errors can be caught and status reset.
    async fn enable_inner(
        &self,
        trainer_info: &TrainerInfo,
        app_id: &str,
        game_pid: Option<u32>,
    ) -> Result<EnableResult> {
        // Download phase
        {
            let mut state = self.state.write().await;
            state.status = SharkDeckStatus::Downloading;
            state.progress = Some("Resolving download link...".to_string());
        }

        let client = self.client().await;

        // Source-aware download: GCW uses its own URL resolution chain and may
        // deliver .rar archives that need extraction. Fling delivers .exe directly.
        //
        // GCW's resolved URLs (mobiletarget.net) are time-limited tokens. If the
        // download body read fails (timeout, connection reset), we must re-resolve
        // a fresh URL — retrying the same stale URL will always fail.
        let trainer_path = if trainer_info.source == "gcw" {
            let mut last_error: Option<anyhow::Error> = None;
            let mut gcw_path: Option<String> = None;

            for attempt in 1u64..=3 {
                if attempt > 1 {
                    let delay = std::time::Duration::from_secs(attempt * 2);
                    self.set_progress(&format!("Retrying download (attempt {}/3)...", attempt)).await;
                    tokio::time::sleep(delay).await;
                }

                // Re-resolve on every attempt — URLs are time-limited tokens
                self.set_progress(&format!("Resolving download link{}...", if attempt > 1 { " (retry)" } else { " (1/3)" })).await;
                let resolved_url = match gcw::resolve_download_url(&client, &trainer_info.download_url).await {
                    Ok(url) => url,
                    Err(e) => {
                        let msg = e.to_string();
                        if msg.contains("Cloudflare") || msg.contains("anti-bot") {
                            last_error = Some(e);
                            continue;
                        }
                        return Err(e);
                    }
                };

                self.set_progress("Downloading trainer file...").await;
                // Pass original URL too — the resolved mobiletarget.net URL has no .rar
                // hint, but the original dl.gamecopyworld.com URL contains "!rar".
                match trainer::download_and_extract_trainer(
                    &client,
                    trainer_info,
                    &resolved_url,
                    Some(&trainer_info.download_url),
                )
                .await {
                    Ok(path) => {
                        gcw_path = Some(path);
                        break;
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        // Clear bad cached files so next attempt re-downloads
                        if msg.contains("HTML instead of") || msg.contains("not a valid RAR") {
                            let safe_name = trainer::sanitize_filename(&trainer_info.name);
                            if let Ok(cache) = std::env::var("HOME") {
                                let bad_file = std::path::PathBuf::from(cache)
                                    .join(".local/share/sharkdeck/cache/trainers")
                                    .join(format!("{}.rar", safe_name));
                                tokio::fs::remove_file(&bad_file).await.ok();
                            }
                        }
                        warn!(attempt = attempt, error = %e, "GCW download attempt failed");
                        last_error = Some(e);
                    }
                }
            }

            match gcw_path {
                Some(path) => path,
                None => {
                    let err = last_error.unwrap_or_else(|| anyhow::anyhow!("GCW download failed after 3 attempts"));
                    let msg = err.to_string();
                    if msg.contains("Cloudflare") || msg.contains("anti-bot") || msg.contains("HTML instead of") {
                        anyhow::bail!("GCW download blocked by anti-bot protection. Try a FLiNG trainer instead.");
                    }
                    anyhow::bail!("GCW download failed after 3 attempts: {}", msg);
                }
            }
        } else {
            self.set_progress("Resolving download link...").await;
            let resolved = fling::resolve_download_url(&client, &trainer_info.download_url).await?;
            self.set_progress("Downloading trainer file...").await;
            trainer::download_trainer(&client, trainer_info, &resolved.download_url).await?
        };

        // Write trainer config file for trainer-hook.sh
        self.set_progress("Saving trainer config...").await;
        save_trainer_config(app_id, &trainer_path, trainer_info).await?;

        // Build the launch options string
        let launch_options = build_launch_options();

        {
            let mut state = self.state.write().await;
            state.status = SharkDeckStatus::Idle;
            state.progress = None;
        }

        info!(
            app_id = %app_id,
            trainer = %trainer_path,
            "trainer enabled for game"
        );

        Ok(EnableResult {
            trainer_path,
            launch_options,
            needs_restart: game_pid.is_some(),
        })
    }

    /// Disables the trainer for the specified game.
    pub async fn disable(&self, app_id: &str) -> Result<()> {
        remove_trainer_config(app_id).await?;
        let mut state = self.state.write().await;
        state.current_trainer = None;
        state.error = None;
        info!(app_id = %app_id, "trainer disabled for game");
        Ok(())
    }

    /// Checks if a trainer is enabled for the given app_id.
    pub async fn get_enabled(&self, app_id: &str) -> Option<TrainerConfig> {
        load_trainer_config(app_id).await
    }

    /// Returns the current SharkDeck status.
    pub async fn status(&self) -> SharkDeckStatusInfo {
        let state = self.state.read().await;
        SharkDeckStatusInfo {
            status: state.status.clone(),
            current_trainer: state.current_trainer.as_ref().map(|t| TrainerSummary {
                name: t.name.clone(),
                game_name: t.game_name.clone(),
                version: t.version.clone(),
            }),
            error: state.error.clone(),
            progress: state.progress.clone(),
        }
    }

    /// Stops any active operation (download, deps install).
    pub async fn stop(&self) -> Result<()> {
        let mut state = self.state.write().await;
        state.status = SharkDeckStatus::Idle;
        state.progress = None;
        info!("sharkdeck operation stopped");
        Ok(())
    }
}

/// Returns the trainers config directory (`~/.config/sharkdeck/trainers/`).
fn trainers_config_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home).join(".config/sharkdeck/trainers"))
}

/// Saves a trainer config file for trainer-hook.sh to read.
async fn save_trainer_config(
    app_id: &str,
    trainer_path: &str,
    trainer_info: &TrainerInfo,
) -> Result<()> {
    let dir = trainers_config_dir()?;
    tokio::fs::create_dir_all(&dir).await?;

    let config = TrainerConfig {
        path: trainer_path.to_string(),
        name: trainer_info.name.clone(),
        game_name: trainer_info.game_name.clone(),
        version: trainer_info.version.clone(),
    };

    let json = serde_json::to_string_pretty(&config)?;
    let config_path = dir.join(format!("{}.json", app_id));
    tokio::fs::write(&config_path, json).await?;
    info!(path = %config_path.display(), "trainer config saved");
    Ok(())
}

/// Removes the trainer config file for a game.
async fn remove_trainer_config(app_id: &str) -> Result<()> {
    let dir = trainers_config_dir()?;
    let config_path = dir.join(format!("{}.json", app_id));
    if tokio::fs::metadata(&config_path).await.is_ok() {
        tokio::fs::remove_file(&config_path).await?;
        info!(path = %config_path.display(), "trainer config removed");
    }
    Ok(())
}

/// Loads a trainer config file if it exists.
async fn load_trainer_config(app_id: &str) -> Option<TrainerConfig> {
    let dir = trainers_config_dir().ok()?;
    let config_path = dir.join(format!("{}.json", app_id));
    let data = tokio::fs::read_to_string(&config_path).await.ok()?;
    serde_json::from_str(&data).ok()
}

/// Builds the launch options string users need to add in Steam.
///
/// This is a universal hook — set it once per game and enable/disable
/// trainers through SharkDeck's UI without touching Steam again.
fn build_launch_options() -> String {
    "/home/deck/.local/share/sharkdeck/trainer-hook.sh %command%".to_string()
}
