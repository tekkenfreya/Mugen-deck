use axum::extract::Path;
use axum::Json;
use serde_json::{json, Value};

use crate::error::AppError;

/// GET /updates/check — checks for available updates.
pub async fn check_updates() -> Json<Value> {
    // Phase 1: stub — no real update mechanism yet
    Json(json!({
        "ok": true,
        "data": {
            "available": [],
        }
    }))
}

/// POST /updates/apply/:id — applies a specific update.
pub async fn apply_update(Path(id): Path<String>) -> Result<Json<Value>, AppError> {
    // Phase 1: stub
    Err(AppError::NotFound(format!(
        "no update found with id '{}'",
        id
    )))
}
