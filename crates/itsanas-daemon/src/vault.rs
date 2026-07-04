//! The vault: a named-file view on top of `itsanas-storage`'s
//! content-addressed shards, encrypted end-to-end.
//!
//! Each file gets its own randomly generated key (wrapped by the account's
//! master key, D10), so the master key never directly touches bulk data.
//! The manifest mapping file names to their wrapped key and ordered chunk
//! list is itself encrypted at rest with the master key, so nothing about
//! a locked vault — not even file names — is readable without it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use itsanas_chunking::{chunk, verify_chunk, ChunkId, DEFAULT_CHUNK_SIZE};
use itsanas_crypto::cipher;
use itsanas_storage::StorageRoot;
use serde::{Deserialize, Serialize};

use crate::error::DaemonError;
use crate::hex::{decode as hex_decode_raw, encode as hex_encode};

const MANIFEST_FILE: &str = "manifest.enc";
const MANIFEST_AAD: &[u8] = b"itsanas-manifest-v1";

#[derive(Serialize, Deserialize, Default)]
struct Manifest {
    files: BTreeMap<String, FileEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct FileEntry {
    wrapped_key: String,
    chunk_ids: Vec<String>,
    size: u64,
}

/// Summary of a stored file, safe to return to a client.
#[derive(Serialize)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
}

pub struct Vault {
    storage: StorageRoot,
    manifest_path: PathBuf,
}

impl Vault {
    pub fn open(data_dir: &Path) -> Result<Self, DaemonError> {
        let storage = StorageRoot::open(data_dir.join("shards"))?;
        Ok(Self {
            storage,
            manifest_path: data_dir.join(MANIFEST_FILE),
        })
    }

    pub fn list(&self, master_key: &[u8; 32]) -> Result<Vec<FileInfo>, DaemonError> {
        let manifest = self.load_manifest(master_key)?;
        Ok(manifest
            .files
            .into_iter()
            .map(|(name, entry)| FileInfo {
                name,
                size: entry.size,
            })
            .collect())
    }

    pub fn put(&self, master_key: &[u8; 32], name: &str, data: &[u8]) -> Result<(), DaemonError> {
        let file_key = itsanas_crypto::wrap::generate_file_key();
        let ciphertext = cipher::encrypt(&file_key, data, name.as_bytes());

        let mut chunk_ids = Vec::new();
        for c in chunk(&ciphertext, DEFAULT_CHUNK_SIZE) {
            self.storage.put(&c.id, &c.data)?;
            chunk_ids.push(c.id.to_string());
        }

        let wrapped_key = hex_encode(itsanas_crypto::wrap::wrap_key(master_key, &file_key));

        let mut manifest = self.load_manifest(master_key)?;
        manifest.files.insert(
            name.to_string(),
            FileEntry {
                wrapped_key,
                chunk_ids,
                size: data.len() as u64,
            },
        );
        self.save_manifest(master_key, &manifest)
    }

    pub fn get(&self, master_key: &[u8; 32], name: &str) -> Result<Vec<u8>, DaemonError> {
        let manifest = self.load_manifest(master_key)?;
        let entry = manifest
            .files
            .get(name)
            .ok_or_else(|| DaemonError::FileNotFound(name.to_string()))?;

        let wrapped_key = hex_decode(&entry.wrapped_key)?;
        let file_key: [u8; 32] = itsanas_crypto::wrap::unwrap_key(master_key, &wrapped_key)
            .map_err(|e| DaemonError::Corrupt(e.to_string()))?;

        let mut ciphertext = Vec::with_capacity(entry.size as usize);
        for hex_id in &entry.chunk_ids {
            let id_bytes: [u8; 32] = hex_decode(hex_id)?
                .try_into()
                .map_err(|_| DaemonError::Corrupt("bad chunk id length".to_string()))?;
            let id = ChunkId::from_bytes(id_bytes);
            let data = self.storage.get(&id)?;
            verify_chunk(&data, &id).map_err(|e| DaemonError::Corrupt(e.to_string()))?;
            ciphertext.extend_from_slice(&data);
        }

        cipher::decrypt(&file_key, &ciphertext, name.as_bytes())
            .map_err(|_| DaemonError::Corrupt("file decryption failed".to_string()))
    }

    pub fn delete(&self, master_key: &[u8; 32], name: &str) -> Result<(), DaemonError> {
        let mut manifest = self.load_manifest(master_key)?;
        let entry = manifest
            .files
            .remove(name)
            .ok_or_else(|| DaemonError::FileNotFound(name.to_string()))?;
        for hex_id in entry.chunk_ids {
            if let Ok(bytes) = hex_decode(&hex_id) {
                if let Ok(id_bytes) = <[u8; 32]>::try_from(bytes) {
                    let _ = self.storage.delete(&ChunkId::from_bytes(id_bytes));
                }
            }
        }
        self.save_manifest(master_key, &manifest)
    }

    fn load_manifest(&self, master_key: &[u8; 32]) -> Result<Manifest, DaemonError> {
        if !self.manifest_path.exists() {
            return Ok(Manifest::default());
        }
        let ciphertext = std::fs::read(&self.manifest_path)?;
        let plaintext = cipher::decrypt(master_key, &ciphertext, MANIFEST_AAD)
            .map_err(|_| DaemonError::WrongPassword)?;
        Ok(serde_json::from_slice(&plaintext)?)
    }

    fn save_manifest(&self, master_key: &[u8; 32], manifest: &Manifest) -> Result<(), DaemonError> {
        let plaintext = serde_json::to_vec(manifest)?;
        let ciphertext = cipher::encrypt(master_key, &plaintext, MANIFEST_AAD);
        std::fs::write(&self.manifest_path, ciphertext)?;
        Ok(())
    }
}

fn hex_decode(s: &str) -> Result<Vec<u8>, DaemonError> {
    hex_decode_raw(s).map_err(DaemonError::Corrupt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> [u8; 32] {
        [7u8; 32]
    }

    #[test]
    fn put_then_get_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        let data = b"hello vault".repeat(500);

        vault.put(&key(), "notes.txt", &data).unwrap();
        let fetched = vault.get(&key(), "notes.txt").unwrap();

        assert_eq!(fetched, data);
    }

    #[test]
    fn list_reports_stored_files() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        vault.put(&key(), "a.txt", b"aaa").unwrap();
        vault.put(&key(), "b.txt", b"bbbbb").unwrap();

        let mut files = vault.list(&key()).unwrap();
        files.sort_by(|a, b| a.name.cmp(&b.name));

        assert_eq!(files[0].name, "a.txt");
        assert_eq!(files[0].size, 3);
        assert_eq!(files[1].name, "b.txt");
        assert_eq!(files[1].size, 5);
    }

    #[test]
    fn get_missing_file_fails() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        assert!(matches!(
            vault.get(&key(), "missing.txt"),
            Err(DaemonError::FileNotFound(_))
        ));
    }

    #[test]
    fn delete_removes_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        vault.put(&key(), "gone.txt", b"bye").unwrap();

        vault.delete(&key(), "gone.txt").unwrap();

        assert!(matches!(
            vault.get(&key(), "gone.txt"),
            Err(DaemonError::FileNotFound(_))
        ));
        assert!(vault.list(&key()).unwrap().is_empty());
    }

    #[test]
    fn wrong_master_key_cannot_read_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        vault.put(&key(), "secret.txt", b"top secret").unwrap();

        let wrong_key = [9u8; 32];
        assert!(matches!(
            vault.get(&wrong_key, "secret.txt"),
            Err(DaemonError::WrongPassword)
        ));
    }
}
