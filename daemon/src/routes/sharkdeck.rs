use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::error::AppError;
use crate::sharkdeck::types::TrainerInfo;
use crate::AppState;

/// Request body for POST /sharkdeck/search.
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub game: String,
}

/// Request body for POST /sharkdeck/enable.
#[derive(Debug, Deserialize)]
pub struct EnableRequest {
    pub trainer: TrainerInfo,
    pub app_id: String,
}

/// Request body for POST /sharkdeck/disable.
#[derive(Debug, Deserialize)]
pub struct DisableRequest {
    pub app_id: String,
}

/// Request body for GET /sharkdeck/enabled (query param).
#[derive(Debug, Deserialize)]
pub struct EnabledQuery {
    pub app_id: String,
}

/// POST /sharkdeck/search — search for trainers matching a game name.
pub async fn search(
    State(state): State<AppState>,
    Json(body): Json<SearchRequest>,
) -> Result<Json<Value>, AppError> {
    if body.game.is_empty() {
        return Err(AppError::BadRequest("game name is required".to_string()));
    }

    match state.sharkdeck.search(&body.game).await {
        Ok(result) => Ok(Json(json!({
            "ok": true,
            "data": result
        }))),
        Err(e) => {
            warn!(error = %e, "sharkdeck search failed");
            Ok(Json(json!({
                "ok": false,
                "error": e.to_string()
            })))
        }
    }
}

/// POST /sharkdeck/enable — start trainer download in background.
///
/// Returns immediately. The frontend polls `/sharkdeck/status` for progress.
/// This prevents long downloads from blocking the HTTP request (which caused
/// Chrome tab suspension to silently kill the connection).
pub async fn enable(
    State(state): State<AppState>,
    Json(body): Json<EnableRequest>,
) -> Result<Json<Value>, AppError> {
    if body.app_id.is_empty() {
        return Err(AppError::BadRequest("app_id is required".to_string()));
    }

    let game_pid = {
        let game = state.game_detector.current_game.read().await;
        game.as_ref().map(|g| g.pid)
    };

    info!(app_id = %body.app_id, "starting trainer enable");
    state
        .sharkdeck
        .start_enable(body.trainer, body.app_id, game_pid)
        .await;

    Ok(Json(json!({
        "ok": true,
        "data": { "started": true }
    })))
}

/// POST /sharkdeck/disable — remove trainer config for a game.
pub async fn disable(
    State(state): State<AppState>,
    Json(body): Json<DisableRequest>,
) -> Result<Json<Value>, AppError> {
    if body.app_id.is_empty() {
        return Err(AppError::BadRequest("app_id is required".to_string()));
    }

    match state.sharkdeck.disable(&body.app_id).await {
        Ok(()) => Ok(Json(json!({
            "ok": true,
            "data": { "disabled": true }
        }))),
        Err(e) => {
            warn!(error = %e, "sharkdeck disable failed");
            Ok(Json(json!({
                "ok": false,
                "error": e.to_string()
            })))
        }
    }
}

/// POST /sharkdeck/enabled — check if a trainer is enabled for a game.
pub async fn enabled(
    State(state): State<AppState>,
    Json(body): Json<EnabledQuery>,
) -> Result<Json<Value>, AppError> {
    if body.app_id.is_empty() {
        return Err(AppError::BadRequest("app_id is required".to_string()));
    }

    let config = state.sharkdeck.get_enabled(&body.app_id).await;
    Ok(Json(json!({
        "ok": true,
        "data": config
    })))
}

/// POST /sharkdeck/cancel — cancel any active download/install operation.
pub async fn cancel(State(state): State<AppState>) -> Json<Value> {
    match state.sharkdeck.stop().await {
        Ok(()) => {
            info!("sharkdeck operation cancelled by user");
            Json(json!({
                "ok": true,
                "data": { "cancelled": true }
            }))
        }
        Err(e) => {
            warn!(error = %e, "sharkdeck cancel failed");
            Json(json!({
                "ok": false,
                "error": e.to_string()
            }))
        }
    }
}

/// GET /sharkdeck/status — get current SharkDeck status.
pub async fn status(State(state): State<AppState>) -> Json<Value> {
    let info = state.sharkdeck.status().await;
    Json(json!({
        "ok": true,
        "data": info
    }))
}

