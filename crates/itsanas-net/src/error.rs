use thiserror::Error;

/// Errors from `itsanas-net`.
#[derive(Debug, Error)]
pub enum NetError {
    /// The QUIC/iroh transport failed (connect, bind, stream I/O). Carries
    /// the underlying error's message rather than the error itself, since
    /// iroh's transport error types don't all uniformly implement traits
    /// convenient for wrapping here.
    #[error("network transport error: {0}")]
    Transport(String),

    /// A peer sent bytes that don't parse as a request or response.
    #[error("protocol error: {0}")]
    Protocol(String),

    /// The remote peer's storage layer reported an error while handling a
    /// request.
    #[error("remote peer reported an error: {0}")]
    Remote(String),

    /// The requested shard is not present on the remote peer.
    #[error("shard not found on remote peer")]
    NotFound,

    /// Local storage operation failed while serving a request.
    #[error("local storage error: {0}")]
    Storage(#[from] itsanas_storage::StorageError),

    /// A shard received from a peer does not hash to the id it was
    /// requested under (D7's hostile-backend assumption extends to
    /// hostile/compromised peers: the network is not a trust boundary
    /// either, so content is re-verified on receipt).
    #[error("shard received from peer failed verification: {0}")]
    Verify(#[from] itsanas_chunking::VerifyError),
}
