//! Background sync engine: mirrors a real local folder with the encrypted
//! vault, so that drag-and-drop, copy/paste, and "open" in the OS's normal
//! file manager work transparently — no manual upload/download step.
//!
//! This is deliberately a *mirrored folder*, not a virtual filesystem/FUSE
//! mount (that's an explicit non-goal for now per the project brief). It
//! watches a flat directory (no subfolders yet) and keeps it reconciled
//! with the vault, both on filesystem events and on a fixed poll interval
//! — the poll is what catches vault-side changes made through the HTTP API
//! directly, which the filesystem watcher can't see.
//!
//! A small plaintext state file records the content hash each file had the
//! last time both sides agreed. That's what tells a one-sided change
//! (either "edited in the folder" or "PUT through the API") apart from a
//! genuine two-sided conflict, where the folder side wins — local edits
//! should behave like editing a normal synced folder, not silently lose
//! data to a background process.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

use crate::state::SharedState;

const STATE_FILE: &str = "sync_state.json";
const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Serialize, Deserialize, Default, Clone)]
struct SyncState {
    /// File name -> BLAKE3 hex hash of its content at the last point both
    /// the folder and the vault agreed on it.
    last_synced: BTreeMap<String, String>,
}

impl SyncState {
    fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|raw| serde_json::from_slice(&raw).ok())
            .unwrap_or_default()
    }

    fn save(&self, path: &Path) {
        if let Ok(raw) = serde_json::to_vec(self) {
            let _ = std::fs::write(path, raw);
        }
    }
}

fn hash(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}

/// Runs forever, mirroring `sync_dir` against the vault. Meant to be
/// spawned as its own background task alongside the HTTP server.
pub async fn run(state: SharedState, sync_dir: PathBuf) {
    if let Err(e) = std::fs::create_dir_all(&sync_dir) {
        eprintln!(
            "sync: failed to create synced folder {}: {e}",
            sync_dir.display()
        );
        return;
    }

    let state_path = state.data_dir.join(STATE_FILE);
    let (tx, rx) = std_mpsc::channel::<()>();

    let mut watcher =
        match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if res.is_ok() {
                let _ = tx.send(());
            }
        }) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("sync: failed to start filesystem watcher: {e}");
                return;
            }
        };
    if let Err(e) = watcher.watch(&sync_dir, RecursiveMode::NonRecursive) {
        eprintln!("sync: failed to watch {}: {e}", sync_dir.display());
        return;
    }

    loop {
        if let Ok(master_key) = state.master_key().await {
            reconcile(&state, &master_key, &sync_dir, &state_path).await;
        }
        tokio::task::block_in_place(|| {
            let _ = rx.recv_timeout(POLL_INTERVAL);
            while rx.try_recv().is_ok() {}
        });
    }
}

async fn reconcile(state: &SharedState, master_key: &[u8; 32], sync_dir: &Path, state_path: &Path) {
    let mut sync_state = SyncState::load(state_path);

    let vault_files: BTreeSet<String> = match state.vault.list(master_key) {
        Ok(files) => files.into_iter().map(|f| f.name).collect(),
        Err(e) => {
            eprintln!("sync: failed to list vault: {e}");
            return;
        }
    };
    let folder_files = list_folder(sync_dir);

    let mut names: BTreeSet<String> = vault_files.clone();
    names.extend(folder_files.iter().cloned());
    names.extend(sync_state.last_synced.keys().cloned());

    let mut changed = false;
    for name in names {
        let local_path = sync_dir.join(&name);
        let in_vault = vault_files.contains(&name);
        let in_folder = folder_files.contains(&name);
        let last_hash = sync_state.last_synced.get(&name).cloned();

        match (in_vault, in_folder) {
            (false, false) => {
                if sync_state.last_synced.remove(&name).is_some() {
                    changed = true;
                }
            }
            (true, false) => {
                if last_hash.is_some() {
                    // Was mirrored locally before, now removed from the
                    // folder by the user -> delete from the vault too.
                    if let Err(e) = state.vault.delete(master_key, &name) {
                        eprintln!("sync: failed to delete {name} from vault: {e}");
                        continue;
                    }
                    sync_state.last_synced.remove(&name);
                    changed = true;
                } else if materialize(state, master_key, &name, &local_path, &mut sync_state).await
                {
                    changed = true;
                }
            }
            (false, true) => {
                if last_hash.is_some() {
                    // Was in the vault before, now gone -> drop the local
                    // copy so the folder reflects a delete made elsewhere.
                    let _ = std::fs::remove_file(&local_path);
                    sync_state.last_synced.remove(&name);
                    changed = true;
                } else if upload(state, master_key, &name, &local_path, &mut sync_state).await {
                    changed = true;
                }
            }
            (true, true) => {
                let local_data = std::fs::read(&local_path).ok();
                let local_hash = local_data.as_deref().map(hash);
                let folder_changed = local_hash.as_deref() != last_hash.as_deref();

                if !folder_changed {
                    match state.vault.get(master_key, &name) {
                        Ok(vault_data) => {
                            let vault_hash = hash(&vault_data);
                            if Some(vault_hash.as_str()) != last_hash.as_deref()
                                && std::fs::write(&local_path, &vault_data).is_ok()
                            {
                                sync_state.last_synced.insert(name, vault_hash);
                                changed = true;
                            }
                        }
                        Err(e) => eprintln!("sync: failed to read {name} from vault: {e}"),
                    }
                } else if let (Some(data), Some(h)) = (local_data, local_hash) {
                    // Folder side changed (a local conflict, if the vault
                    // side changed too, resolves in favor of the folder —
                    // it should behave like a normal editable folder).
                    if let Err(e) = state.vault.put(master_key, &name, &data) {
                        eprintln!("sync: failed to upload {name}: {e}");
                    } else {
                        sync_state.last_synced.insert(name, h);
                        changed = true;
                    }
                }
            }
        }
    }

    if changed {
        sync_state.save(state_path);
    }
}

async fn materialize(
    state: &SharedState,
    master_key: &[u8; 32],
    name: &str,
    local_path: &Path,
    sync_state: &mut SyncState,
) -> bool {
    match state.vault.get(master_key, name) {
        Ok(data) => {
            let h = hash(&data);
            if std::fs::write(local_path, &data).is_ok() {
                sync_state.last_synced.insert(name.to_string(), h);
                true
            } else {
                false
            }
        }
        Err(e) => {
            eprintln!("sync: failed to read {name} from vault: {e}");
            false
        }
    }
}

async fn upload(
    state: &SharedState,
    master_key: &[u8; 32],
    name: &str,
    local_path: &Path,
    sync_state: &mut SyncState,
) -> bool {
    match std::fs::read(local_path) {
        Ok(data) => {
            let h = hash(&data);
            if let Err(e) = state.vault.put(master_key, name, &data) {
                eprintln!("sync: failed to upload {name}: {e}");
                false
            } else {
                sync_state.last_synced.insert(name.to_string(), h);
                true
            }
        }
        Err(e) => {
            eprintln!("sync: failed to read local file {name}: {e}");
            false
        }
    }
}

fn list_folder(dir: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    if !name.starts_with('.') {
                        out.insert(name.to_string());
                    }
                }
            }
        }
    }
    out
}
