//! Background daemon: exposes the local, authenticated HTTP API that
//! `itsanas-gui` and the Android client (D9) talk to.
//!
//! Ties `itsanas-crypto`, `itsanas-chunking`, and `itsanas-storage` into a
//! single-user encrypted vault ([`vault::Vault`]) behind a password-derived
//! master key ([`account`]), a folder-sync engine ([`sync`]), and
//! background scrubbing ([`scrub`], D7's active detection half —
//! `itsanas-repair`'s mirroring/recovery half isn't wired in yet, since
//! that needs a real mirror-peer list, a multi-device accounts (M4)
//! question).

mod account;
mod error;
mod hex;
pub mod http;
pub mod scrub;
mod state;
pub mod sync;
mod vault;

pub use error::DaemonError;
pub use state::AppState;
