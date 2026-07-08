use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::RwLock;

use crate::account;
use crate::error::DaemonError;
use crate::vault::{Vault, VaultHealth};

/// A single file the sync engine could not reconcile on its most recent
/// pass — e.g. its name couldn't be read from the synced folder, or the
/// vault rejected it. Surfaced through `/status` so a stuck file shows up
/// in the GUI instead of retrying invisibly forever (that silent-retry
/// shape is exactly how the Windows os-error-5 shard-write bug went
/// unnoticed: the daemon logged nothing a user would ever see).
#[derive(Serialize, Clone)]
pub struct SyncIssue {
    /// The best available name for the affected file. Falls back to a
    /// lossy rendering when the real name isn't valid UTF-8.
    pub name: String,
    pub message: String,
}

/// Shared server state: the vault, and the master key while unlocked.
///
/// Bound to `127.0.0.1` only (see `main.rs`) — this is a local daemon for
/// one user's own clients (the GUI, the Android app talking to it over a
/// tunnel), not a multi-tenant server, so "unlocked in memory for the
/// process lifetime" is an appropriate trust boundary for now. Stronger
/// session/token handling is an M4 (accounts) concern.
pub struct AppState {
    pub data_dir: PathBuf,
    pub sync_dir: PathBuf,
    pub vault: Vault,
    master_key: RwLock<Option<[u8; 32]>>,
    /// The most recent background scrub result (`scrub::run`), if any has
    /// completed yet. `None` before the first scrub, or whenever the
    /// vault is locked — a locked vault reveals nothing, including
    /// whether any of its files are healthy.
    vault_health: RwLock<Option<VaultHealth>>,
    /// Files the sync engine failed to reconcile on its most recent pass.
    /// Replaced wholesale each pass (see `sync::reconcile`), so a file
    /// that starts working again drops off automatically.
    sync_issues: RwLock<Vec<SyncIssue>>,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    pub fn open(data_dir: PathBuf, sync_dir: PathBuf) -> Result<Self, DaemonError> {
        let vault = Vault::open(&data_dir)?;
        Ok(Self {
            data_dir,
            sync_dir,
            vault,
            master_key: RwLock::new(None),
            vault_health: RwLock::new(None),
            sync_issues: RwLock::new(Vec::new()),
        })
    }

    pub fn has_account(&self) -> bool {
        account::exists(&self.data_dir)
    }

    pub async fn setup(&self, password: &str) -> Result<(), DaemonError> {
        let key = account::setup(&self.data_dir, password)?;
        *self.master_key.write().await = Some(key);
        Ok(())
    }

    pub async fn unlock(&self, password: &str) -> Result<(), DaemonError> {
        let key = account::unlock(&self.data_dir, password)?;
        *self.master_key.write().await = Some(key);
        Ok(())
    }

    pub async fn lock(&self) {
        *self.master_key.write().await = None;
        // A locked vault reveals nothing — including whether its files
        // were healthy as of the last scrub, or which ones the sync
        // engine was struggling with.
        *self.vault_health.write().await = None;
        self.sync_issues.write().await.clear();
    }

    pub async fn is_unlocked(&self) -> bool {
        self.master_key.read().await.is_some()
    }

    pub async fn master_key(&self) -> Result<[u8; 32], DaemonError> {
        self.master_key.read().await.ok_or(DaemonError::Locked)
    }

    pub async fn vault_health(&self) -> Option<VaultHealth> {
        self.vault_health.read().await.clone()
    }

    pub async fn set_vault_health(&self, health: VaultHealth) {
        *self.vault_health.write().await = Some(health);
    }

    pub async fn sync_issues(&self) -> Vec<SyncIssue> {
        self.sync_issues.read().await.clone()
    }

    pub async fn set_sync_issues(&self, issues: Vec<SyncIssue>) {
        *self.sync_issues.write().await = issues;
    }
}
