//! Local storage root management and shard I/O.
//!
//! Treats the underlying storage backend as hostile/unreliable (D7):
//! [`StorageRoot::put`] does write-then-verify-readback instead of trusting
//! `fsync` (which can lie over a network filesystem like SMB), and
//! [`StorageRoot::get`] re-verifies a shard's content address on every
//! read, so tampering or corruption that happens between writes is caught
//! rather than silently returned to the caller.

mod error;
mod hex;
mod root;

pub use error::StorageError;
pub use root::StorageRoot;
