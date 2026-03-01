use anyhow::{Context, Result};
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use std::path::PathBuf;
use tracing::info;
use uuid::Uuid;

use crate::AppState;

/// Returns the path to the session token file.
fn token_path() -> Result<PathBuf> {
    let config_dir = crate::config::config_dir()?;
    Ok(config_dir.join("session.token"))
}

/// Generates a new UUID v4 session token and writes it to disk.
pub fn generate_session_token() -> Result<String> {
    let token = Uuid::new_v4().to_string();
    let path = token_path()?;

    std::fs::write(&path, &token)
        .with_context(|| format!("failed to write session token to {}", path.display()))?;

    info!("session token generated");
    Ok(token)
}

/// Reads the current session token from disk.
pub fn read_session_token() -> Result<String> {
    let path = token_path()?;
    let token = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read session token from {}", path.display()))?;
    Ok(token.trim().to_string())
}

/// Axum middleware that validates the `Authorization: Bearer <token>` header.
pub async fn auth_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| unauthorized_response("missing authorization header"))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| unauthorized_response("invalid authorization format"))?;

    if token != state.session_token.as_str() {
        return Err(unauthorized_response("invalid token"));
    }

    Ok(next.run(request).await)
}

/// Builds a 401 JSON response.
fn unauthorized_response(message: &str) -> Response {
    let body = json!({
        "ok": false,
        "error": message,
    });

    (StatusCode::UNAUTHORIZED, axum::Json(body)).into_response()
}
