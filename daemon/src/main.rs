use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use mugen_daemon::app_manager::AppManager;
use mugen_daemon::game_detection::GameDetector;
use mugen_daemon::{auth, config, routes, AppState};
use tokio::net::TcpListener;
use tokio::signal;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Load config
    let cfg = config::load()?;

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| cfg.log_level.clone().into()),
        )
        .init();

    info!(version = env!("CARGO_PKG_VERSION"), "mugen-daemon starting");

    // Ensure all directories exist
    config::ensure_dirs()?;

    // Generate session token
    let session_token = auth::generate_session_token()?;

    // Initialize game detection
    let game_detector = GameDetector::new();
    let detector_handle = game_detector.clone().start_polling();

    // Initialize app manager
    let app_manager = AppManager::new();
    app_manager.load_manifests().await?;

    let state = AppState {
        session_token: Arc::new(session_token),
        started_at: Instant::now(),
        game_detector,
        app_manager,
    };

    // Build router
    let app = routes::router(state);

    // Bind to localhost only
    let addr: SocketAddr = format!("{}:{}", cfg.host, cfg.port).parse()?;
    let listener = TcpListener::bind(addr).await?;
    info!(%addr, "listening");

    // Serve with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Clean up
    detector_handle.abort();
    info!("daemon shut down gracefully");
    Ok(())
}

/// Waits for SIGINT or SIGTERM to initiate graceful shutdown.
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("received SIGINT"),
        _ = terminate => info!("received SIGTERM"),
    }
}
