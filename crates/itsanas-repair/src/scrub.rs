//! Scrubbing: the active detection half of D7. Re-verifies shards against
//! `itsanas-storage`'s own verify-on-read rather than trusting a directory
//! listing, since the storage backend is assumed hostile/unreliable —
//! bytes that were fine when written can still be tampered with or
//! corrupted later, and a scrub is exactly the periodic check that
//! catches that instead of waiting for a read that happens to need them.

use itsanas_chunking::ChunkId;
use itsanas_storage::{StorageError, StorageRoot};

/// The health of one shard, as observed by [`scrub`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShardStatus {
    Healthy,
    /// Present but failed verification, or couldn't be read at all — both
    /// mean this shard can't be trusted and needs [`crate::repair`].
    Corrupt,
    Missing,
}

/// The result of scrubbing a set of shard ids against a [`StorageRoot`].
#[derive(Debug)]
pub struct ScrubReport {
    pub statuses: Vec<(ChunkId, ShardStatus)>,
}

impl ScrubReport {
    /// Every shard that needs repair (anything not [`ShardStatus::Healthy`]).
    pub fn unhealthy(&self) -> impl Iterator<Item = &ChunkId> {
        self.statuses
            .iter()
            .filter(|(_, status)| *status != ShardStatus::Healthy)
            .map(|(id, _)| id)
    }
}

/// Re-verifies every shard in `ids` against `storage`, classifying each as
/// healthy, corrupt, or missing.
pub fn scrub(storage: &StorageRoot, ids: &[ChunkId]) -> ScrubReport {
    let statuses = ids
        .iter()
        .map(|id| {
            let status = match storage.get(id) {
                Ok(_) => ShardStatus::Healthy,
                Err(StorageError::NotFound(_)) => ShardStatus::Missing,
                // A read failure (I/O error) is just as untrustworthy as a
                // verification failure for scrubbing's purposes — either
                // way this shard can't currently be served correctly.
                Err(_) => ShardStatus::Corrupt,
            };
            (*id, status)
        })
        .collect();
    ScrubReport { statuses }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> (tempfile::TempDir, StorageRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = StorageRoot::open(dir.path()).unwrap();
        (dir, root)
    }

    fn corrupt_on_disk(dir: &std::path::Path, id: &ChunkId) {
        let hex = id.to_string();
        let path = dir.join("shards").join(&hex[0..2]).join(&hex);
        std::fs::write(&path, b"corrupted content!!").unwrap();
    }

    #[test]
    fn scrub_reports_healthy_corrupt_and_missing_shards() {
        let (dir, root) = temp_root();

        let healthy_data = b"a perfectly fine shard";
        let healthy_id = ChunkId::of(healthy_data);
        root.put(&healthy_id, healthy_data).unwrap();

        let corrupt_data = b"about to be tampered with";
        let corrupt_id = ChunkId::of(corrupt_data);
        root.put(&corrupt_id, corrupt_data).unwrap();
        corrupt_on_disk(dir.path(), &corrupt_id);

        let missing_id = ChunkId::of(b"never stored");

        let report = scrub(&root, &[healthy_id, corrupt_id, missing_id]);

        assert_eq!(
            report.statuses,
            vec![
                (healthy_id, ShardStatus::Healthy),
                (corrupt_id, ShardStatus::Corrupt),
                (missing_id, ShardStatus::Missing),
            ]
        );
        let unhealthy: Vec<ChunkId> = report.unhealthy().copied().collect();
        assert_eq!(unhealthy, vec![corrupt_id, missing_id]);
    }
}
