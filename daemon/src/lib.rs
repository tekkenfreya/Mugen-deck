pub mod app_manager;
pub mod auth;
pub mod config;
pub mod error;
pub mod game_detection;
pub mod routes;
pub mod sharkdeck;

use std::sync::Arc;
use std::time::Instant;

/// Shared application state passed to all route handlers.
#[derive(Clone)]
pub struct AppState {
    pub session_token: Arc<String>,
    pub started_at: Instant,
    pub game_detector: game_detection::GameDetector,
    pub app_manager: app_manager::AppManager,
    pub sharkdeck: sharkdeck::SharkDeckManager,
}
