use itsanas_chunking::chunk;
use itsanas_crypto::{cipher, kdf, wrap};
use itsanas_net::{NetError, Node};
use itsanas_storage::StorageRoot;

const AAD: &[u8] = b"itsanas-receipt-scenario";

/// Runs the M1 two-node LAN scenario once: encrypt a small file, chunk the
/// ciphertext, push every chunk to a peer, then recover it by fetching
/// every chunk back (as if the local copy were lost), verify, decrypt, and
/// compare to the original.
///
/// This is the same pipeline as `itsanas-net`'s
/// `tests/lan_store_retrieve.rs`, reused here as a standalone binary so
/// `scripts/receipt.sh` can drive it under fault injection.
pub async fn run() -> Result<(), NetError> {
    let owner_dir = tempfile::tempdir().expect("tempdir");
    let mirror_dir = tempfile::tempdir().expect("tempdir");
    let owner = Node::bind(StorageRoot::open(owner_dir.path()).expect("open storage")).await?;
    let mirror = Node::bind(StorageRoot::open(mirror_dir.path()).expect("open storage")).await?;

    let mirror_addr = mirror.addr();
    tokio::spawn(mirror.clone().serve());

    let plaintext = b"the quick brown fox jumps over the lazy dog".repeat(200);
    let salt = kdf::generate_salt();
    let master_key = kdf::derive_master_key(b"correct horse battery staple", &salt).expect("kdf");
    let file_key = wrap::generate_file_key();
    let _wrapped_file_key = wrap::wrap_key(&master_key, &file_key);
    let ciphertext = cipher::encrypt(&file_key, &plaintext, AAD);

    let chunks = chunk(&ciphertext, 4096);

    for c in &chunks {
        owner
            .put_remote(mirror_addr.clone(), &c.id, &c.data)
            .await?;
    }

    let mut recovered_ciphertext = Vec::new();
    for c in &chunks {
        let data = owner.get_remote(mirror_addr.clone(), &c.id).await?;
        recovered_ciphertext.extend_from_slice(&data);
    }

    if recovered_ciphertext != ciphertext {
        return Err(NetError::Protocol(
            "recovered ciphertext did not match what was stored".to_string(),
        ));
    }

    let recovered_plaintext = cipher::decrypt(&file_key, &recovered_ciphertext, AAD)
        .map_err(|_| NetError::Protocol("decryption of recovered ciphertext failed".to_string()))?;

    if recovered_plaintext != plaintext {
        return Err(NetError::Protocol(
            "recovered plaintext did not match the original".to_string(),
        ));
    }

    Ok(())
}
