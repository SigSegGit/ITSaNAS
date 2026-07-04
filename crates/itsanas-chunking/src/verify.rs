use thiserror::Error;

use crate::chunk_id::ChunkId;

/// Errors from verifying chunk data against its expected content address.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum VerifyError {
    /// The bytes read back do not hash to the expected `ChunkId` — the
    /// storage backend corrupted, truncated, or tampered with the shard
    /// (D7). Carries both hashes so callers (e.g. the future scrubbing job)
    /// can log which shard failed and re-fetch/re-replicate it.
    #[error("chunk verification failed: expected {expected}, got {actual}")]
    Mismatch { expected: ChunkId, actual: ChunkId },
}

/// Verifies that `data` is exactly the chunk identified by `expected`.
///
/// This is the single code path used both for normal reads and for the
/// scrubbing job that periodically re-hashes stored shards (D7): there is
/// one place that decides whether a chunk's bytes are trustworthy.
pub fn verify(data: &[u8], expected: &ChunkId) -> Result<(), VerifyError> {
    let actual = ChunkId::of(data);
    if actual == *expected {
        Ok(())
    } else {
        Err(VerifyError::Mismatch {
            expected: *expected,
            actual,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_untampered_data() {
        let data = b"a stored shard";
        let id = ChunkId::of(data);
        assert!(verify(data, &id).is_ok());
    }

    #[test]
    fn detects_corruption() {
        let data = b"a stored shard";
        let id = ChunkId::of(data);
        let corrupted = b"a st0red shard";
        assert!(verify(corrupted, &id).is_err());
    }

    #[test]
    fn detects_truncation() {
        let data = b"a stored shard";
        let id = ChunkId::of(data);
        assert!(verify(&data[..data.len() - 1], &id).is_err());
    }

    #[test]
    fn mismatch_error_carries_both_hashes() {
        let data = b"a stored shard";
        let id = ChunkId::of(data);
        let corrupted = b"a st0red shard";
        match verify(corrupted, &id) {
            Err(VerifyError::Mismatch { expected, actual }) => {
                assert_eq!(expected, id);
                assert_eq!(actual, ChunkId::of(corrupted));
                assert_ne!(expected, actual);
            }
            Ok(()) => panic!("expected verification to fail"),
        }
    }
}
