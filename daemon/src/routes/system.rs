use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};

use crate::error::AppError;
use crate::AppState;

/// GET /system/stats — returns basic system statistics.
pub async fn system_stats(State(state): State<AppState>) -> Json<Value> {
    let uptime = state.started_at.elapsed().as_secs();

    Json(json!({
        "ok": true,
        "data": {
            "daemon_uptime_secs": uptime,
            "apps_loaded": state.app_manager.list_apps().await.len(),
        }
    }))
}

/// POST /system/profile/:game_id — stub for per-game profile management.
pub async fn set_profile(Path(game_id): Path<String>) -> Result<Json<Value>, AppError> {
    // Phase 1: stub
    Err(AppError::NotFound(format!(
        "profile management not implemented for '{}'",
        game_id
    )))
}
