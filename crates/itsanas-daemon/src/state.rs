use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::account;
use crate::error::DaemonError;
use crate::vault::{Vault, VaultHealth};

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
        // were healthy as of the last scrub.
        *self.vault_health.write().await = None;
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
}
