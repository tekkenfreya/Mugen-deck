pub mod apps;
pub mod game;
pub mod health;
pub mod system;
pub mod updates;

use axum::middleware;
use axum::routing::get;
use axum::routing::post;
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use crate::auth::auth_middleware;
use crate::AppState;

/// Assembles all daemon routes.
///
/// - `/health` is public (no auth).
/// - `/ui` serves the launcher frontend (no auth).
/// - All other routes require Bearer token auth.
/// - CORS rejects all origins — daemon is localhost-only.
pub fn router(state: AppState) -> Router {
    // Serve frontend static files from ~/.local/share/mugen/launcher/ui/
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/deck".to_string());
    let ui_dir = format!("{}/.local/share/mugen/launcher/ui", home);

    // CORS: reject all cross-origin requests — daemon is localhost-only
    let cors = CorsLayer::new();

    let public = Router::new()
        .route("/health", get(health::health))
        .nest_service("/ui", ServeDir::new(&ui_dir).append_index_html_on_directories(true))
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

    public.merge(protected).layer(cors)
}
