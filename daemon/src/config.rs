use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;
use tracing::info;

/// Daemon configuration loaded from TOML.
#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    7331
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            log_level: default_log_level(),
        }
    }
}

/// Returns the base config directory: `~/.config/mugen/`.
pub fn config_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME environment variable not set")?;
    Ok(PathBuf::from(home).join(".config").join("mugen"))
}

/// Returns the base data directory: `~/.local/share/mugen/`.
pub fn data_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME environment variable not set")?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("mugen"))
}

/// Ensures all required Mugen directories exist.
pub fn ensure_dirs() -> Result<()> {
    let config = config_dir()?;
    let data = data_dir()?;

    let dirs = [
        config.clone(),
        config.join("apps"),
        data.join("launcher"),
        data.join("apps"),
        data.join("profiles"),
        data.join("logs"),
        data.join("cache"),
    ];

    for dir in &dirs {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create directory: {}", dir.display()))?;
    }

    info!("all directories verified");
    Ok(())
}

/// Loads configuration from `~/.config/mugen/config.toml`.
/// Returns default config if file does not exist.
pub fn load() -> Result<Config> {
    let config_path = config_dir()?.join("config.toml");

    if !config_path.exists() {
        info!("no config file found, using defaults");
        return Ok(Config::default());
    }

    let contents = std::fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read config: {}", config_path.display()))?;

    let config: Config = toml::from_str(&contents).context("failed to parse config.toml")?;

    info!(path = %config_path.display(), "loaded configuration");
    Ok(config)
}
