//! Background daemon: exposes the local, authenticated HTTP API that
//! `itsanas-gui` and the Android client (D9) talk to.
//!
//! Ties `itsanas-crypto`, `itsanas-chunking`, and `itsanas-storage` into a
//! single-user encrypted vault ([`vault::Vault`]) behind a password-derived
//! master key ([`account`]). Peer-to-peer sync (`itsanas-net`) and
//! multi-device accounts (M4) are not wired in yet — this milestone is the
//! local vault + API surface both clients need to exist at all.

mod account;
mod error;
mod hex;
pub mod http;
mod state;
pub mod sync;
mod vault;

pub use error::DaemonError;
pub use state::AppState;
