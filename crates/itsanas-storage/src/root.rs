use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use itsanas_chunking::{verify_chunk, ChunkId};

use crate::error::StorageError;
use crate::hex;

/// A local, content-addressed shard store rooted at a directory.
///
/// Treats the underlying filesystem as hostile/unreliable (D7): every
/// [`StorageRoot::put`] writes to a temp file, syncs it, atomically renames
/// it into place, then **reads the bytes back and re-verifies their hash**
/// before returning success — `fsync` alone isn't trusted, since it can lie
/// over network filesystems like SMB. Every [`StorageRoot::get`] re-verifies
/// on the way out too, so a shard tampered with or corrupted after it was
/// written is caught on next read rather than silently returned.
pub struct StorageRoot {
    root: PathBuf,
}

impl StorageRoot {
    /// Opens (creating if necessary) a storage root at `root`.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let root = root.into();
        fs::create_dir_all(shards_dir(&root))?;
        Ok(Self { root })
    }

    /// Stores `data` under `id`, verifying on read-back before returning
    /// (D7). If `data` doesn't actually hash to `id`, that's a caller bug,
    /// not a storage-backend fault — this is asserted via `verify_chunk`
    /// failing loudly rather than silently mislabeling a shard.
    pub fn put(&self, id: &ChunkId, data: &[u8]) -> Result<(), StorageError> {
        let final_path = self.path_for(id);
        fs::create_dir_all(final_path.parent().expect("shard path always has a parent"))?;

        let tmp_path = self.root.join(format!(
            "shards/.tmp-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is after the epoch")
                .as_nanos()
        ));
        fs::write(&tmp_path, data)?;
        {
            let f = fs::File::open(&tmp_path)?;
            f.sync_all()?;
        }
        fs::rename(&tmp_path, &final_path)?;

        let written_back = fs::read(&final_path)?;
        if let Err(err) = verify_chunk(&written_back, id) {
            let _ = fs::remove_file(&final_path);
            return Err(StorageError::Verify(err));
        }

        Ok(())
    }

    /// Reads the shard stored under `id`, verifying its hash before
    /// returning it (D7).
    pub fn get(&self, id: &ChunkId) -> Result<Vec<u8>, StorageError> {
        let path = self.path_for(id);
        if !path.exists() {
            return Err(StorageError::NotFound(*id));
        }
        let data = fs::read(&path)?;
        verify_chunk(&data, id)?;
        Ok(data)
    }

    /// Removes the shard stored under `id`. Removing a shard that isn't
    /// present is not an error.
    pub fn delete(&self, id: &ChunkId) -> Result<(), StorageError> {
        let path = self.path_for(id);
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Whether a shard is stored under `id`. Note this only checks
    /// presence, not integrity — use [`StorageRoot::get`] to also verify.
    pub fn contains(&self, id: &ChunkId) -> bool {
        self.path_for(id).exists()
    }

    /// Lists the `ChunkId`s of every shard currently stored.
    pub fn list(&self) -> Result<Vec<ChunkId>, StorageError> {
        let mut ids = Vec::new();
        for shard_dir in fs::read_dir(shards_dir(&self.root))? {
            let shard_dir = shard_dir?.path();
            if !shard_dir.is_dir() {
                continue;
            }
            for entry in fs::read_dir(&shard_dir)? {
                let entry = entry?;
                let name = entry.file_name();
                let Some(name) = name.to_str() else {
                    continue;
                };
                if let Some(id) = parse_chunk_id(name) {
                    ids.push(id);
                }
            }
        }
        Ok(ids)
    }

    fn path_for(&self, id: &ChunkId) -> PathBuf {
        let hex = id.to_string();
        shards_dir(&self.root).join(&hex[0..2]).join(hex)
    }
}

fn shards_dir(root: &std::path::Path) -> PathBuf {
    root.join("shards")
}

fn parse_chunk_id(hex_name: &str) -> Option<ChunkId> {
    let bytes = hex::decode(hex_name)?;
    let bytes: [u8; 32] = bytes.try_into().ok()?;
    Some(ChunkId::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> (tempfile::TempDir, StorageRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = StorageRoot::open(dir.path()).unwrap();
        (dir, root)
    }

    #[test]
    fn put_then_get_round_trips() {
        let (_dir, root) = temp_root();
        let data = b"a shard's worth of ciphertext";
        let id = ChunkId::of(data);

        root.put(&id, data).unwrap();
        let got = root.get(&id).unwrap();

        assert_eq!(got, data);
    }

    #[test]
    fn get_missing_shard_returns_not_found() {
        let (_dir, root) = temp_root();
        let id = ChunkId::of(b"never stored");
        assert!(matches!(root.get(&id), Err(StorageError::NotFound(_))));
    }

    #[test]
    fn contains_reflects_presence() {
        let (_dir, root) = temp_root();
        let data = b"present";
        let id = ChunkId::of(data);

        assert!(!root.contains(&id));
        root.put(&id, data).unwrap();
        assert!(root.contains(&id));
    }

    #[test]
    fn delete_removes_a_stored_shard() {
        let (_dir, root) = temp_root();
        let data = b"to be deleted";
        let id = ChunkId::of(data);
        root.put(&id, data).unwrap();

        root.delete(&id).unwrap();

        assert!(!root.contains(&id));
        assert!(matches!(root.get(&id), Err(StorageError::NotFound(_))));
    }

    #[test]
    fn delete_of_missing_shard_is_not_an_error() {
        let (_dir, root) = temp_root();
        let id = ChunkId::of(b"was never here");
        assert!(root.delete(&id).is_ok());
    }

    #[test]
    fn put_is_idempotent_for_identical_content() {
        let (_dir, root) = temp_root();
        let data = b"stored twice";
        let id = ChunkId::of(data);

        root.put(&id, data).unwrap();
        root.put(&id, data).unwrap();

        assert_eq!(root.get(&id).unwrap(), data);
    }

    #[test]
    fn list_returns_every_stored_id() {
        let (_dir, root) = temp_root();
        let a = ChunkId::of(b"chunk a");
        let b = ChunkId::of(b"chunk b");
        root.put(&a, b"chunk a").unwrap();
        root.put(&b, b"chunk b").unwrap();

        let mut listed = root.list().unwrap();
        listed.sort();
        let mut expected = vec![a, b];
        expected.sort();

        assert_eq!(listed, expected);
    }

    #[test]
    fn list_is_empty_for_a_fresh_root() {
        let (_dir, root) = temp_root();
        assert!(root.list().unwrap().is_empty());
    }

    /// D7: a hostile/unreliable storage backend can silently corrupt a
    /// shard after it's written (bad disk, tampering, a lossy SMB mount).
    /// `get` must catch this rather than returning the wrong bytes.
    #[test]
    fn get_detects_a_shard_corrupted_on_disk_after_writing() {
        let (dir, root) = temp_root();
        let data = b"untampered content";
        let id = ChunkId::of(data);
        root.put(&id, data).unwrap();

        let hex = id.to_string();
        let path = dir.path().join("shards").join(&hex[0..2]).join(&hex);
        fs::write(&path, b"corrupted content!!").unwrap();

        assert!(matches!(root.get(&id), Err(StorageError::Verify(_))));
    }
}
