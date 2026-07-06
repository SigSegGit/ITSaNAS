//! The local HTTP API: everything `itsanas-gui` and the Android client
//! speak to. Deliberately small and JSON/raw-bytes based rather than a
//! bespoke binary protocol, since both clients are thin and this is the
//! easiest thing for them to consume.

use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::DaemonError;
use crate::state::SharedState;
use crate::vault::VaultHealth;

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/status", get(status))
        .route("/account/setup", axum::routing::post(setup))
        .route("/account/unlock", axum::routing::post(unlock))
        .route("/account/lock", axum::routing::post(lock))
        .route("/files", get(list_files))
        .route(
            "/files/{name}",
            put(put_file).get(get_file).delete(delete_file),
        )
        // This is a loopback-only daemon talking to its own trusted
        // clients (the GUI, the Android app over a tunnel), not a public
        // upload endpoint — axum's 2 MiB default would silently break any
        // file bigger than that, which defeats the point of a synced
        // folder that's supposed to behave like a normal filesystem.
        .layer(DefaultBodyLimit::disable())
        .with_state(state)
}

#[derive(Serialize)]
struct StatusResponse {
    has_account: bool,
    unlocked: bool,
    synced_folder: String,
    /// The most recent background scrub result (D7), if one has run yet.
    /// Always `null` while locked — a locked vault reveals nothing.
    vault_health: Option<VaultHealth>,
}

async fn status(State(state): State<SharedState>) -> impl IntoResponse {
    Json(StatusResponse {
        has_account: state.has_account(),
        unlocked: state.is_unlocked().await,
        synced_folder: state.sync_dir.display().to_string(),
        vault_health: state.vault_health().await,
    })
}

#[derive(Deserialize)]
struct PasswordRequest {
    password: String,
}

async fn setup(
    State(state): State<SharedState>,
    Json(req): Json<PasswordRequest>,
) -> Result<StatusCode, DaemonError> {
    state.setup(&req.password).await?;
    Ok(StatusCode::CREATED)
}

async fn unlock(
    State(state): State<SharedState>,
    Json(req): Json<PasswordRequest>,
) -> Result<StatusCode, DaemonError> {
    state.unlock(&req.password).await?;
    Ok(StatusCode::OK)
}

async fn lock(State(state): State<SharedState>) -> impl IntoResponse {
    state.lock().await;
    StatusCode::OK
}

async fn list_files(State(state): State<SharedState>) -> Result<impl IntoResponse, DaemonError> {
    let key = state.master_key().await?;
    let files = state.vault.list(&key)?;
    Ok(Json(files))
}

async fn put_file(
    State(state): State<SharedState>,
    Path(name): Path<String>,
    body: axum::body::Bytes,
) -> Result<StatusCode, DaemonError> {
    let key = state.master_key().await?;
    state.vault.put(&key, &name, &body)?;
    Ok(StatusCode::CREATED)
}

async fn get_file(
    State(state): State<SharedState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, DaemonError> {
    let key = state.master_key().await?;
    let data = state.vault.get(&key, &name)?;
    Ok(([("content-type", "application/octet-stream")], data))
}

async fn delete_file(
    State(state): State<SharedState>,
    Path(name): Path<String>,
) -> Result<StatusCode, DaemonError> {
    let key = state.master_key().await?;
    state.vault.delete(&key, &name)?;
    Ok(StatusCode::NO_CONTENT)
}
