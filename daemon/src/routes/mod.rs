pub mod apps;
pub mod auth;
pub mod game;
pub mod health;
pub mod sharkdeck;
pub mod system;
pub mod updates;

use axum::middleware;
use axum::routing::get;
use axum::routing::post;
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

use crate::auth::auth_middleware;
use crate::AppState;

/// Assembles all daemon routes.
///
/// - `/health` is public (no auth).
/// - `/ui` serves the launcher frontend (no auth).
/// - All other routes require Bearer token auth.
/// - CORS rejects all origins — daemon is localhost-only.
pub fn router(state: AppState) -> Router {
    // Serve frontend static files from ~/.local/share/sharkdeck/launcher/ui/
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/deck".to_string());
    let ui_dir = format!("{}/.local/share/sharkdeck/launcher/ui", home);

    // CORS: reject all cross-origin requests — daemon is localhost-only
    let cors = CorsLayer::new();

    let public = Router::new()
        .route("/health", get(health::health))
        .route("/auth/token", get(auth::get_token))
        .nest_service(
            "/ui",
            ServeDir::new(&ui_dir)
                .append_index_html_on_directories(true)
                .fallback(ServeFile::new(format!("{}/index.html", ui_dir))),
        )
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
        // SharkDeck
        .route("/sharkdeck/search", post(sharkdeck::search))
        .route("/sharkdeck/enable", post(sharkdeck::enable))
        .route("/sharkdeck/disable", post(sharkdeck::disable))
        .route("/sharkdeck/enabled", post(sharkdeck::enabled))
        .route("/sharkdeck/cancel", post(sharkdeck::cancel))
        .route("/sharkdeck/status", get(sharkdeck::status))
        .route("/sharkdeck/hotkey", post(sharkdeck::hotkey))
        // System
        .route("/system/stats", get(system::system_stats))
        .route("/system/profile/{game_id}", post(system::set_profile))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);

    public
        .merge(protected)
        .layer(cors)
        .layer(middleware::from_fn(no_cache))
}

/// Adds no-cache headers to all responses.
///
/// Prevents Chrome from serving stale frontend code after updates.
/// Safe for localhost — no CDN or performance penalty.
async fn no_cache(req: axum::extract::Request, next: middleware::Next) -> axum::response::Response {
    let mut resp = next.run(req).await;
    resp.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        "no-cache, no-store, must-revalidate".parse().unwrap(),
    );
    resp
}
