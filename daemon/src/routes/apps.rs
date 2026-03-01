use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};

use crate::app_manager::AppStatus;
use crate::error::AppError;
use crate::AppState;

/// GET /apps — returns all registered apps.
pub async fn list_apps(State(state): State<AppState>) -> Json<Value> {
    let apps = state.app_manager.list_apps().await;
    Json(json!({
        "ok": true,
        "data": apps,
    }))
}

/// POST /apps/:id/launch — launches an app.
pub async fn launch_app(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let app = state
        .app_manager
        .get_app(&id)
        .await
        .ok_or_else(|| AppError::NotFound(format!("app '{}' not found", id)))?;

    if app.status == AppStatus::Running {
        return Err(AppError::BadRequest(format!(
            "app '{}' is already running",
            id
        )));
    }

    state
        .app_manager
        .set_status(&id, AppStatus::Running)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(json!({
        "ok": true,
        "data": { "id": id, "status": "running" },
    })))
}

/// POST /apps/:id/close — closes a running app.
pub async fn close_app(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let app = state
        .app_manager
        .get_app(&id)
        .await
        .ok_or_else(|| AppError::NotFound(format!("app '{}' not found", id)))?;

    if app.status != AppStatus::Running {
        return Err(AppError::BadRequest(format!("app '{}' is not running", id)));
    }

    state
        .app_manager
        .set_status(&id, AppStatus::Stopped)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(json!({
        "ok": true,
        "data": { "id": id, "status": "stopped" },
    })))
}
