use itsanas_chunking::{ChunkId, VerifyError};
use thiserror::Error;

/// Errors from reading, writing, or listing shards in a [`crate::StorageRoot`].
#[derive(Debug, Error)]
pub enum StorageError {
    /// The underlying filesystem operation failed.
    #[error("storage I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A shard's bytes did not hash to its expected `ChunkId` (D7): either
    /// tampering, corruption, or truncation by the storage backend.
    #[error("shard verification failed: {0}")]
    Verify(#[from] VerifyError),

    /// No shard is stored under this `ChunkId`.
    #[error("no shard stored for {0}")]
    NotFound(ChunkId),
}
