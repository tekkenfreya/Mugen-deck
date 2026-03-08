use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// App manifest as defined in `cc-app.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub entry: String,
}

/// Runtime info for a managed app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    #[serde(flatten)]
    pub manifest: AppManifest,
    pub status: AppStatus,
}

/// App lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AppStatus {
    Installed,
    Running,
    Stopped,
}

/// Manages app lifecycle and manifests.
#[derive(Debug, Clone)]
pub struct AppManager {
    apps: Arc<RwLock<HashMap<String, AppInfo>>>,
}

impl Default for AppManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AppManager {
    /// Creates a new app manager.
    pub fn new() -> Self {
        Self {
            apps: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Loads app manifests from `~/.config/sharkdeck/apps/`.
    pub async fn load_manifests(&self) -> Result<()> {
        let apps_dir = crate::config::config_dir()?.join("apps");
        if !apps_dir.exists() {
            info!("no apps directory found, skipping manifest load");
            return Ok(());
        }

        let entries = std::fs::read_dir(&apps_dir)
            .with_context(|| format!("failed to read {}", apps_dir.display()))?;

        let mut loaded = HashMap::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("cc-app.json");
                if manifest_path.exists() {
                    match load_manifest(&manifest_path) {
                        Ok(manifest) => {
                            let id = manifest.id.clone();
                            loaded.insert(
                                id.clone(),
                                AppInfo {
                                    manifest,
                                    status: AppStatus::Installed,
                                },
                            );
                            info!(app_id = %id, "loaded app manifest");
                        }
                        Err(e) => {
                            warn!(
                                path = %manifest_path.display(),
                                error = %e,
                                "failed to load manifest"
                            );
                        }
                    }
                }
            }
        }

        *self.apps.write().await = loaded;
        info!(count = self.apps.read().await.len(), "app manifests loaded");
        Ok(())
    }

    /// Returns all registered apps.
    pub async fn list_apps(&self) -> Vec<AppInfo> {
        self.apps.read().await.values().cloned().collect()
    }

    /// Returns a specific app by ID.
    pub async fn get_app(&self, id: &str) -> Option<AppInfo> {
        self.apps.read().await.get(id).cloned()
    }

    /// Sets the status of an app.
    pub async fn set_status(&self, id: &str, status: AppStatus) -> Result<()> {
        let mut apps = self.apps.write().await;
        let app = apps.get_mut(id).context("app not found")?;
        app.status = status;
        Ok(())
    }
}

/// Loads and parses a single `cc-app.json` manifest.
fn load_manifest(path: &PathBuf) -> Result<AppManifest> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: AppManifest =
        serde_json::from_str(&content).context("failed to parse cc-app.json")?;
    Ok(manifest)
}
