pub mod apps;
pub mod game;
pub mod health;
pub mod system;
pub mod updates;

use axum::middleware;
use axum::routing::{get, post};
use axum::Router;

use crate::auth::auth_middleware;
use crate::AppState;

/// Assembles all daemon routes.
///
/// - `/health` is public (no auth).
/// - All other routes require Bearer token auth.
pub fn router(state: AppState) -> Router {
    let public = Router::new()
        .route("/health", get(health::health))
        .with_state(state.clone());

    let protected = Router::new()
        // Apps
        .route("/apps", get(apps::list_apps))
        .route("/apps/{id}/launch", post(apps::launch_app))
        .route("/apps/{id}/close", post(apps::close_app))
        // Game detection
        .route("/game/current", get(game::current_game))
        .route("/game/library", get(game::game_library))
        // Updates
        .route("/updates/check", get(updates::check_updates))
        .route("/updates/apply/{id}", post(updates::apply_update))
        // System
        .route("/system/stats", get(system::system_stats))
        .route("/system/profile/{game_id}", post(system::set_profile))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);

    public.merge(protected)
}
