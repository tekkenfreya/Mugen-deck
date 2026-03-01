use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::AppState;

/// GET /health — returns daemon status, version, and uptime.
pub async fn health(State(state): State<AppState>) -> Json<Value> {
    let uptime_secs = state.started_at.elapsed().as_secs();

    Json(json!({
        "ok": true,
        "data": {
            "status": "running",
            "version": env!("CARGO_PKG_VERSION"),
            "uptime_secs": uptime_secs,
        }
    }))
}
