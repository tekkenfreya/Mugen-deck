use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use mugen_daemon::app_manager::AppManager;
use mugen_daemon::game_detection::GameDetector;
use mugen_daemon::sharkdeck::SharkDeckManager;
use mugen_daemon::AppState;
use tokio::net::TcpListener;

/// Starts the daemon on a random port and returns the bound address + token.
async fn spawn_server() -> (SocketAddr, String) {
    let token = "test-token-12345".to_string();

    let state = AppState {
        session_token: Arc::new(token.clone()),
        started_at: Instant::now(),
        game_detector: GameDetector::new(),
        app_manager: AppManager::new(),
        sharkdeck: SharkDeckManager::new(),
    };

    let app = mugen_daemon::routes::router(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (addr, token)
}

#[tokio::test]
async fn health_returns_ok() {
    let (addr, _) = spawn_server().await;
    let url = format!("http://{}/health", addr);

    let resp = reqwest::get(&url).await.unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["ok"], true);
    assert_eq!(body["data"]["status"], "running");
    assert!(body["data"]["version"].is_string());
    assert!(body["data"]["uptime_secs"].is_number());
}

#[tokio::test]
async fn protected_route_requires_auth() {
    let (addr, _) = spawn_server().await;
    let url = format!("http://{}/apps", addr);

    // No auth header → 401
    let resp = reqwest::get(&url).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn protected_route_accepts_valid_token() {
    let (addr, token) = spawn_server().await;
    let url = format!("http://{}/apps", addr);

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["ok"], true);
}
