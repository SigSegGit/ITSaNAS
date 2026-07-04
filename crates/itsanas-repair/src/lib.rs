//! Redundancy and repair: mirroring, erasure coding, and scrubbing.
//!
//! Implements D6 (encrypted mirroring below 4 nodes, Reed–Solomon erasure
//! coding at 4+ nodes with a migration path, ≤3× contribution overhead) and
//! the active half of D7 (scheduled scrubbing that re-hashes shards via
//! `itsanas-chunking`'s verify-on-read, flags tampering/corruption/deletion,
//! and triggers repair; consuming `itsanas-storage`'s permission/ownership
//! monitoring to mark a node "degraded" and proactively re-replicate).
//!
//! Placeholder crate: no implementation yet. Mirroring lands at M3;
//! Reed–Solomon erasure coding is M7 (post-prototype, N≥4).
