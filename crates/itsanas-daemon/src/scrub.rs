//! Background scrubbing (D7's active detection half): periodically
//! re-verifies every shard the vault's manifest references, so bitrot or
//! tampering that happens *after* a file was written is caught
//! proactively — not just the next time (if ever) someone happens to
//! read that particular file.
//!
//! This is deliberately scoped to detection only for now. Recovering a
//! flagged file requires fetching a good copy from a mirror peer
//! (`itsanas_repair::repair`), which in turn requires knowing who this
//! vault's mirror peers even are — a multi-device/account question that
//! belongs to M4, not this milestone. Wiring `repair` in here today would
//! mean inventing a throwaway peer-configuration mechanism just to have
//! something to call; better to surface real, honest health information
//! now (so a user at least knows a file needs attention) and wire actual
//! recovery in once M4 gives this a real peer list to draw on.

use std::time::Duration;

use crate::state::SharedState;

const DEFAULT_SCRUB_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Runs forever, scrubbing the vault on a fixed interval (configurable
/// via `ITSANAS_SCRUB_INTERVAL_SECS`, mainly so tests don't have to wait
/// hours). Intended to be spawned as its own background task alongside
/// the HTTP server and the folder-sync engine.
pub async fn run(state: SharedState) {
    let interval = std::env::var("ITSANAS_SCRUB_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_SCRUB_INTERVAL);

    loop {
        if let Ok(master_key) = state.master_key().await {
            match state.vault.scrub(&master_key) {
                Ok(health) => {
                    if !health.unhealthy_files.is_empty() {
                        eprintln!(
                            "scrub: {} file(s) need attention: {}",
                            health.unhealthy_files.len(),
                            health.unhealthy_files.join(", ")
                        );
                    }
                    state.set_vault_health(health).await;
                }
                Err(e) => eprintln!("scrub: failed to scrub vault: {e}"),
            }
        }
        tokio::time::sleep(interval).await;
    }
}
