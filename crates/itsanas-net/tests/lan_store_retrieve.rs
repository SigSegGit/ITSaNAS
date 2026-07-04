//! M1 acceptance test: two nodes on a LAN (here, two iroh endpoints on
//! localhost, relaying disabled) store and retrieve an encrypted,
//! content-addressed file across the network.
//!
//! Mirrors the real scenario from the project's acceptance criteria: a node
//! encrypts and chunks a file, pushes the shards to a peer (redundancy),
//! and can later recover the file by fetching those shards back — as if
//! its own local copy were unavailable.

use itsanas_chunking::chunk;
use itsanas_crypto::{cipher, kdf, wrap};
use itsanas_net::{NetError, Node};
use itsanas_storage::StorageRoot;

#[tokio::test]
async fn two_node_lan_put_and_recover() {
    let owner_dir = tempfile::tempdir().unwrap();
    let mirror_dir = tempfile::tempdir().unwrap();
    let owner = Node::bind(StorageRoot::open(owner_dir.path()).unwrap())
        .await
        .unwrap();
    let mirror = Node::bind(StorageRoot::open(mirror_dir.path()).unwrap())
        .await
        .unwrap();

    let mirror_addr = mirror.addr();
    tokio::spawn(mirror.clone().serve());

    // Encrypt a "file" under a per-file key wrapped by a password-derived
    // master key, exactly like the real crypto pipeline (D10).
    let plaintext = b"the quick brown fox jumps over the lazy dog".repeat(1000);
    let salt = kdf::generate_salt();
    let master_key = kdf::derive_master_key(b"correct horse battery staple", &salt).unwrap();
    let file_key = wrap::generate_file_key();
    let _wrapped_file_key = wrap::wrap_key(&master_key, &file_key);
    let ciphertext = cipher::encrypt(&file_key, &plaintext, b"itsanas-file-content");

    // Chunk the ciphertext and push every shard to the mirror peer — this
    // is the "uploads land on the other node" half of the acceptance
    // criteria.
    let chunks = chunk(&ciphertext, 4096);
    assert!(chunks.len() > 1, "test file should span multiple chunks");
    for c in &chunks {
        owner
            .put_remote(mirror_addr.clone(), &c.id, &c.data)
            .await
            .unwrap();
    }

    // Confirm the shards really did land on the mirror's own local storage.
    for c in &chunks {
        assert_eq!(mirror.get_local(&c.id).unwrap(), c.data);
    }

    // Now recover: fetch every shard back from the mirror as if the
    // owner's local copy were gone, reassemble, and decrypt.
    let mut recovered_ciphertext = Vec::new();
    for c in &chunks {
        let data = owner.get_remote(mirror_addr.clone(), &c.id).await.unwrap();
        recovered_ciphertext.extend_from_slice(&data);
    }
    assert_eq!(recovered_ciphertext, ciphertext);

    let recovered_plaintext =
        cipher::decrypt(&file_key, &recovered_ciphertext, b"itsanas-file-content").unwrap();
    assert_eq!(recovered_plaintext, plaintext);
}

#[tokio::test]
async fn get_remote_of_unknown_chunk_returns_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let peer = Node::bind(StorageRoot::open(dir.path()).unwrap())
        .await
        .unwrap();
    let peer_addr = peer.addr();
    tokio::spawn(peer.clone().serve());

    let requester_dir = tempfile::tempdir().unwrap();
    let requester = Node::bind(StorageRoot::open(requester_dir.path()).unwrap())
        .await
        .unwrap();

    let unknown_id = itsanas_chunking::ChunkId::of(b"never stored anywhere");
    let result = requester.get_remote(peer_addr, &unknown_id).await;

    assert!(matches!(result, Err(NetError::NotFound)));
}
