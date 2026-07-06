//! Redundancy and repair: mirroring, scrubbing, and recovery.
//!
//! Implements D6's mirroring policy (encrypted full replication to every
//! peer below the N≥4 threshold where Reed–Solomon erasure coding takes
//! over — that's M7, post-prototype) and the active half of D7: scrubbing
//! re-verifies stored shards ([`scrub`]) so tampering, corruption, or
//! deletion is caught proactively rather than waiting for a read that
//! happens to need that shard, and [`repair`] recovers a flagged shard
//! from a mirror rather than just reporting the problem.

mod mirror;
mod scrub;

pub use mirror::{mirror_shard, repair, MirrorReport, MirrorSet, RepairError};
pub use scrub::{scrub, ScrubReport, ShardStatus};
