use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::error::AppError;
use crate::sharkdeck::types::TrainerInfo;
use crate::AppState;

/// Allowed xdotool key names — prevents arbitrary command injection.
const ALLOWED_KEYS: &[&str] = &[
    "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
    "KP_0", "KP_1", "KP_2", "KP_3", "KP_4", "KP_5", "KP_6", "KP_7", "KP_8", "KP_9",
    "KP_Add", "KP_Subtract", "KP_Multiply", "KP_Divide", "KP_Decimal", "KP_Enter",
    "Num_Lock",
    "Home", "End", "Delete", "Insert", "Print", "Prior", "Next",
];

/// Request body for POST /sharkdeck/hotkey.
#[derive(Debug, Deserialize)]
pub struct HotkeyRequest {
    pub key: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
}

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

/// POST /sharkdeck/hotkey — send a keypress to the trainer window via xdotool.
///
/// Finds the trainer window by searching for Wine/Proton windows with "trainer"
/// in the name, then sends the keypress directly to that window — even if
/// CheatBoard (or any other app) is currently focused.
pub async fn hotkey(
    State(state): State<AppState>,
    Json(body): Json<HotkeyRequest>,
) -> Result<Json<Value>, AppError> {
    if !ALLOWED_KEYS.contains(&body.key.as_str()) {
        return Err(AppError::BadRequest(format!(
            "key '{}' is not in the allowed list",
            body.key
        )));
    }

    let mut parts: Vec<&str> = Vec::new();
    if body.ctrl {
        parts.push("ctrl");
    }
    if body.shift {
        parts.push("shift");
    }
    if body.alt {
        parts.push("alt");
    }
    parts.push(&body.key);
    let key_arg = parts.join("+");

    // Find the trainer window ID. Try multiple search strategies:
    // 1. Search by name containing "trainer" (most FLiNG/GCW trainers)
    // 2. Search by name from the currently enabled trainer config
    // 3. Fall back to the game window itself
    let window_id = find_trainer_window(&state).await;

    match window_id {
        Some(wid) => {
            info!(key = %key_arg, window = %wid, "sending hotkey to trainer window");

            let output = tokio::process::Command::new("xdotool")
                .arg("key")
                .arg("--window")
                .arg(&wid)
                .arg(&key_arg)
                .output()
                .await
                .map_err(|e| AppError::Internal(format!("failed to run xdotool: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!(stderr = %stderr, "xdotool failed");
                return Ok(Json(json!({
                    "ok": false,
                    "error": format!("xdotool failed: {}", stderr.trim())
                })));
            }

            Ok(Json(json!({
                "ok": true,
                "data": { "sent": true, "window": wid }
            })))
        }
        None => {
            warn!(key = %key_arg, "no trainer window found, sending to game window");

            // Fallback: find any Wine/Proton window (the game itself)
            let output = tokio::process::Command::new("xdotool")
                .arg("key")
                .arg(&key_arg)
                .output()
                .await
                .map_err(|e| AppError::Internal(format!("failed to run xdotool: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Ok(Json(json!({
                    "ok": false,
                    "error": format!("xdotool failed: {}", stderr.trim())
                })));
            }

            Ok(Json(json!({
                "ok": true,
                "data": { "sent": true, "window": "active" }
            })))
        }
    }
}

/// Searches for the trainer window using xdotool.
///
/// Tries multiple strategies to find the right window:
/// 1. Windows with "trainer" in the name (case-insensitive)
/// 2. Windows with "fling" in the name
/// 3. Windows matching the enabled trainer's name
async fn find_trainer_window(state: &AppState) -> Option<String> {
    // Strategy 1: Search for windows with "trainer" in the name
    let search_terms = ["trainer", "Trainer", "TRAINER", "fling", "Fling"];

    for term in &search_terms {
        if let Ok(output) = tokio::process::Command::new("xdotool")
            .arg("search")
            .arg("--name")
            .arg(term)
            .output()
            .await
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // xdotool returns one window ID per line; take the first
                if let Some(wid) = stdout.lines().next() {
                    let wid = wid.trim();
                    if !wid.is_empty() {
                        return Some(wid.to_string());
                    }
                }
            }
        }
    }

    // Strategy 2: Search by the enabled trainer's name from config
    let current_game = state.game_detector.current_game.read().await;
    if let Some(game) = current_game.as_ref() {
        if let Some(config) = state.sharkdeck.get_enabled(&game.app_id.to_string()).await {
            // Try the trainer name as a window search term
            if let Ok(output) = tokio::process::Command::new("xdotool")
                .arg("search")
                .arg("--name")
                .arg(&config.name)
                .output()
                .await
            {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if let Some(wid) = stdout.lines().next() {
                        let wid = wid.trim();
                        if !wid.is_empty() {
                            return Some(wid.to_string());
                        }
                    }
                }
            }
        }
    }

    None
}
