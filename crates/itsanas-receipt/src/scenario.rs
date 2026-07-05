use itsanas_chunking::chunk;
use itsanas_crypto::{cipher, kdf, wrap};
use itsanas_net::{NetError, Node, RelayPolicy};
use itsanas_repair::{MirrorSet, RepairError};
use itsanas_storage::StorageRoot;

const AAD: &[u8] = b"itsanas-receipt-scenario";

/// Either half of the scenario can fail: the M1 store/retrieve pipeline
/// (`NetError`) or the M3 mirror+repair step (`RepairError`). Kept as one
/// error type so `main.rs`'s single `report`/`expected_for` can match on
/// exactly which fault point produced which failure.
#[derive(Debug)]
pub enum ScenarioError {
    Net(NetError),
    Repair(RepairError),
}

impl From<NetError> for ScenarioError {
    fn from(e: NetError) -> Self {
        ScenarioError::Net(e)
    }
}

impl From<RepairError> for ScenarioError {
    fn from(e: RepairError) -> Self {
        ScenarioError::Repair(e)
    }
}

impl std::fmt::Display for ScenarioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScenarioError::Net(e) => write!(f, "{e}"),
            ScenarioError::Repair(e) => write!(f, "{e}"),
        }
    }
}

/// Runs the M1 two-node LAN scenario (encrypt a small file, chunk the
/// ciphertext, push every chunk to a peer, recover it by fetching every
/// chunk back as if the local copy were lost, verify, decrypt, compare to
/// the original), then an M3 step: mirror the first chunk to two more
/// nodes and `repair()` it back into the owner's own storage as if the
/// owner's local copy needed recovering.
///
/// The M1 half is the same pipeline as `itsanas-net`'s
/// `tests/lan_store_retrieve.rs`; the M3 half is the same as
/// `itsanas-repair`'s own mirror/repair tests — reused here as a
/// standalone binary so `scripts/receipt.sh` can drive the whole thing
/// under fault injection.
pub async fn run() -> Result<(), ScenarioError> {
    let owner_dir = tempfile::tempdir().expect("tempdir");
    let mirror_dir = tempfile::tempdir().expect("tempdir");
    let owner = Node::bind(
        StorageRoot::open(owner_dir.path()).expect("open storage"),
        RelayPolicy::Disabled,
    )
    .await?;
    let mirror = Node::bind(
        StorageRoot::open(mirror_dir.path()).expect("open storage"),
        RelayPolicy::Disabled,
    )
    .await?;

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
        )
        .into());
    }

    let recovered_plaintext = cipher::decrypt(&file_key, &recovered_ciphertext, AAD)
        .map_err(|_| NetError::Protocol("decryption of recovered ciphertext failed".to_string()))?;

    if recovered_plaintext != plaintext {
        return Err(NetError::Protocol(
            "recovered plaintext did not match the original".to_string(),
        )
        .into());
    }

    // M3 (D6/D7): mirror the first chunk to two more nodes, then repair()
    // it back into the owner's storage as if the owner's local copy
    // needed recovering. Proves the mirror+repair path end-to-end, and
    // gives `repair-all-mirrors-unreachable` a real repair() call to force
    // a failure in.
    let mirror_a_dir = tempfile::tempdir().expect("tempdir");
    let mirror_b_dir = tempfile::tempdir().expect("tempdir");
    let mirror_a = Node::bind(
        StorageRoot::open(mirror_a_dir.path()).expect("open storage"),
        RelayPolicy::Disabled,
    )
    .await?;
    let mirror_b = Node::bind(
        StorageRoot::open(mirror_b_dir.path()).expect("open storage"),
        RelayPolicy::Disabled,
    )
    .await?;
    let mirror_set = MirrorSet::new(vec![mirror_a.addr(), mirror_b.addr()]);
    tokio::spawn(mirror_a.clone().serve());
    tokio::spawn(mirror_b.clone().serve());

    let first_chunk = chunks
        .first()
        .expect("scenario ciphertext spans at least one chunk");
    itsanas_repair::mirror_shard(&owner, &mirror_set, &first_chunk.id, &first_chunk.data).await;
    itsanas_repair::repair(&owner, &mirror_set, &first_chunk.id).await?;

    Ok(())
}
