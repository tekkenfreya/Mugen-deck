use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::AppState;

/// GET /auth/token — returns the current session token.
///
/// This endpoint is public (no auth required) because it's the mechanism
/// by which the launcher obtains the token. Safe because the daemon
/// binds to 127.0.0.1 only.
pub async fn get_token(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "ok": true,
        "data": {
            "token": state.session_token.as_str()
        }
    }))
}
