use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::AppState;

/// GET /game/current — returns the currently running Steam game, if any.
pub async fn current_game(State(state): State<AppState>) -> Json<Value> {
    let current = state.game_detector.current_game.read().await;
    match current.as_ref() {
        Some(game) => Json(json!({
            "ok": true,
            "data": game,
        })),
        None => Json(json!({
            "ok": true,
            "data": null,
        })),
    }
}

/// GET /game/library — returns all detected Steam games.
pub async fn game_library(State(state): State<AppState>) -> Json<Value> {
    let library = state.game_detector.library.read().await;
    Json(json!({
        "ok": true,
        "data": library.as_slice(),
    }))
}
