//! Mirroring and repair: the active-replication half of D6 (below the
//! N≥4 threshold where Reed–Solomon erasure coding takes over, M7) and
//! the corrective half of D7 — once scrubbing (or a normal read) flags a
//! shard as unhealthy, repair fetches a good copy from a mirror instead
//! of just reporting the problem.

use itsanas_chunking::ChunkId;
use itsanas_net::{EndpointAddr, NetError, Node};
use thiserror::Error;

/// The set of peers a node mirrors its shards to. Just a list — D6's
/// policy of "mirror to every other node below 4 total" is a decision
/// for the caller (who knows the current membership); this type doesn't
/// need to know why a given peer is in the set, only that it is.
#[derive(Debug, Clone, Default)]
pub struct MirrorSet {
    pub peers: Vec<EndpointAddr>,
}

impl MirrorSet {
    pub fn new(peers: Vec<EndpointAddr>) -> Self {
        Self { peers }
    }
}

/// The outcome of pushing one shard to every peer in a [`MirrorSet`].
#[derive(Debug)]
pub struct MirrorReport {
    pub succeeded: Vec<EndpointAddr>,
    pub failed: Vec<(EndpointAddr, NetError)>,
}

/// Pushes `data` (already-encrypted shard bytes, keyed by `id`) to every
/// peer in `mirror_set`. A failure on one peer doesn't stop the others —
/// D7's hostile/unreliable-backend assumption applies to mirror peers
/// too, so one bad or unreachable mirror shouldn't block replicating to
/// the rest.
pub async fn mirror_shard(
    node: &Node,
    mirror_set: &MirrorSet,
    id: &ChunkId,
    data: &[u8],
) -> MirrorReport {
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();
    for peer in &mirror_set.peers {
        match node.put_remote(peer.clone(), id, data).await {
            Ok(()) => succeeded.push(peer.clone()),
            Err(e) => failed.push((peer.clone(), e)),
        }
    }
    MirrorReport { succeeded, failed }
}

#[derive(Debug, Error)]
pub enum RepairError {
    #[error("no mirror in the set had a valid copy of {0}")]
    NoHealthyMirror(ChunkId),
    #[error(transparent)]
    Net(#[from] NetError),
}

/// Restores shard `id` into `node`'s own local storage by fetching a copy
/// from the first mirror in `mirror_set` that actually has a valid one.
///
/// Tries every peer before giving up — a mirror being unreachable, or
/// (per D7) serving back a copy that fails verification, is just a
/// reason to try the next candidate, not to fail outright.
/// `Node::get_remote` already re-verifies content on receipt, so a lying
/// or corrupted mirror's response is rejected before we ever get here.
pub async fn repair(node: &Node, mirror_set: &MirrorSet, id: &ChunkId) -> Result<(), RepairError> {
    for peer in &mirror_set.peers {
        #[cfg(feature = "test-mode")]
        if itsanas_testkit::should_fail(itsanas_testkit::FaultPoint::RepairAllMirrorsUnreachable) {
            // Simulate this candidate mirror being unreachable — the loop's
            // real "try the next one, then give up" logic below is exactly
            // what runs, this just forces every attempt to miss.
            continue;
        }

        match node.get_remote(peer.clone(), id).await {
            Ok(data) => {
                node.put_local(id, &data)?;
                return Ok(());
            }
            Err(_) => continue,
        }
    }
    Err(RepairError::NoHealthyMirror(*id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use itsanas_net::RelayPolicy;
    use itsanas_storage::StorageRoot;

    async fn bind_node() -> (tempfile::TempDir, Node) {
        let dir = tempfile::tempdir().unwrap();
        let node = Node::bind(
            StorageRoot::open(dir.path()).unwrap(),
            RelayPolicy::Disabled,
        )
        .await
        .unwrap();
        (dir, node)
    }

    #[tokio::test]
    async fn mirror_shard_pushes_to_every_peer() {
        let (_owner_dir, owner) = bind_node().await;
        let (_m1_dir, mirror1) = bind_node().await;
        let (_m2_dir, mirror2) = bind_node().await;

        let mirror1_addr = mirror1.addr();
        let mirror2_addr = mirror2.addr();
        tokio::spawn(mirror1.clone().serve());
        tokio::spawn(mirror2.clone().serve());

        let data = b"mirrored shard content";
        let id = ChunkId::of(data);
        let mirror_set = MirrorSet::new(vec![mirror1_addr, mirror2_addr]);

        let report = mirror_shard(&owner, &mirror_set, &id, data).await;

        assert_eq!(report.succeeded.len(), 2);
        assert!(report.failed.is_empty());
        assert_eq!(mirror1.get_local(&id).unwrap(), data);
        assert_eq!(mirror2.get_local(&id).unwrap(), data);
    }

    #[tokio::test]
    async fn mirror_shard_reports_partial_failure_without_aborting() {
        let (_owner_dir, owner) = bind_node().await;
        let (_m1_dir, mirror1) = bind_node().await;

        let mirror1_addr = mirror1.addr();
        tokio::spawn(mirror1.clone().serve());

        // An address nobody is serving on — bound, but never spawned, so
        // connecting to it fails like a genuinely offline mirror would.
        let (_dead_dir, dead_mirror) = bind_node().await;
        let dead_addr = dead_mirror.addr();
        drop(dead_mirror);

        let data = b"partially mirrored content";
        let id = ChunkId::of(data);
        let mirror_set = MirrorSet::new(vec![mirror1_addr, dead_addr]);

        let report = mirror_shard(&owner, &mirror_set, &id, data).await;

        assert_eq!(report.succeeded.len(), 1);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(mirror1.get_local(&id).unwrap(), data);
    }

    #[tokio::test]
    async fn repair_recovers_from_a_healthy_mirror_after_a_corrupt_one() {
        let (_owner_dir, owner) = bind_node().await;
        let (m1_dir, mirror1) = bind_node().await;
        let (_m2_dir, mirror2) = bind_node().await;

        let mirror1_addr = mirror1.addr();
        let mirror2_addr = mirror2.addr();
        tokio::spawn(mirror1.clone().serve());
        tokio::spawn(mirror2.clone().serve());

        let data = b"the shard that needs recovering, repeated for length"[..].repeat(10);
        let id = ChunkId::of(&data);
        let mirror_set = MirrorSet::new(vec![mirror1_addr, mirror2_addr]);

        // Seed both mirrors, then directly corrupt mirror1's on-disk copy
        // (bypassing put's own write-then-verify-readback, which would
        // just reject the corruption at write time) — simulating a mirror
        // whose copy degraded after being stored. The owner never has a
        // local copy at all here — the case repair actually exists for.
        let _ = mirror_shard(&owner, &mirror_set, &id, &data).await;
        let hex = id.to_string();
        let corrupt_path = m1_dir.path().join("shards").join(&hex[0..2]).join(&hex);
        std::fs::write(&corrupt_path, b"corrupted!").unwrap();

        repair(&owner, &mirror_set, &id)
            .await
            .expect("repair should fall through the corrupt mirror to the healthy one");

        assert_eq!(owner.get_local(&id).unwrap(), data);
    }

    #[tokio::test]
    async fn repair_fails_cleanly_when_no_mirror_has_the_shard() {
        let (_owner_dir, owner) = bind_node().await;
        let (_m1_dir, mirror1) = bind_node().await;
        tokio::spawn(mirror1.clone().serve());

        let id = ChunkId::of(b"never stored anywhere");
        let mirror_set = MirrorSet::new(vec![mirror1.addr()]);

        let result = repair(&owner, &mirror_set, &id).await;

        assert!(matches!(result, Err(RepairError::NoHealthyMirror(_))));
    }
}
