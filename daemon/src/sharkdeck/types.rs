use serde::{Deserialize, Serialize};

/// Information about a trainer found on a trainer database site.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerInfo {
    /// Display name of the trainer.
    pub name: String,
    /// Game name the trainer is for.
    pub game_name: String,
    /// Trainer version or date string.
    pub version: String,
    /// URL to the trainer page or direct download.
    pub download_url: String,
    /// Expected file size in bytes, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<u64>,
    /// SHA256 checksum for verification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    /// Source website.
    pub source: String,
}

/// Result of a trainer search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The query string used.
    pub query: String,
    /// Matching trainers found.
    pub trainers: Vec<TrainerInfo>,
    /// Source website.
    pub source: String,
}

/// Per-game trainer configuration file (`~/.config/sharkdeck/trainers/<appid>.json`).
///
/// Read by `trainer-hook.sh` at game launch to set `PROTON_REMOTE_DEBUG_CMD`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerConfig {
    /// Absolute path to the trainer executable.
    pub path: String,
    /// Display name of the trainer.
    pub name: String,
    /// Game name the trainer is for.
    pub game_name: String,
    /// Trainer version.
    pub version: String,
}

/// Status of the SharkDeck subsystem.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SharkDeckStatus {
    Idle,
    Searching,
    /// Installing .NET Framework + VC++ Runtime in the game's Wine prefix.
    InstallingDeps,
    Downloading,
    Error,
}

/// Full status info returned by the status endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharkDeckStatusInfo {
    pub status: SharkDeckStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_trainer: Option<TrainerSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Human-readable progress message for the current operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<String>,
}

/// Summary of the currently active trainer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerSummary {
    pub name: String,
    pub game_name: String,
    pub version: String,
}

/// Result of enabling a trainer for a game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnableResult {
    /// Path where the trainer was downloaded.
    pub trainer_path: String,
    /// Launch options string to add in Steam.
    pub launch_options: String,
    /// Whether the game needs to be restarted (was already running).
    pub needs_restart: bool,
}
