use std::path::PathBuf;
use std::sync::Arc;

use itsanas_daemon::{http, sync, AppState};

const DEFAULT_PORT: u16 = 4279;

/// Per-user app-data directory (`%APPDATA%\itsanas` on Windows, `~/.config/itsanas`
/// on Linux, `~/Library/Application Support/itsanas` on macOS). Deliberately not
/// CWD-relative: a Start-Menu/Desktop-launched exe has no reliable working
/// directory, and Program Files isn't writable by a standard user account.
fn default_data_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("itsanas")
}

/// The visible, synced folder, right in the user's home directory so it's
/// easy to find — the same idea as `~/Google Drive` or `~/Dropbox`.
fn default_sync_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ITSaNAS")
}

#[tokio::main]
async fn main() {
    let data_dir = std::env::var("ITSANAS_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_data_dir());
    let port: u16 = std::env::var("ITSANAS_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let sync_dir = std::env::var("ITSANAS_SYNC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_sync_dir());

    let state =
        Arc::new(AppState::open(data_dir.clone(), sync_dir.clone()).expect("failed to open vault"));

    tokio::spawn(sync::run(state.clone(), sync_dir.clone()));

    let app = http::router(state);

    // Loopback only: this is a local daemon for this machine's own clients
    // (the GUI, or the Android app via a tunnel), not a public server.
    let addr = format!("127.0.0.1:{port}");
    println!(
        "itsanas-daemon listening on http://{addr} (data dir: {}, synced folder: {})",
        data_dir.display(),
        sync_dir.display()
    );

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {addr}: {e}"));
    axum::serve(listener, app)
        .await
        .expect("daemon server error");
}
