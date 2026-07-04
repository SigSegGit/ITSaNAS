//! Local storage root management and shard I/O.
//!
//! Treats the underlying storage backend as hostile/unreliable (D7): this
//! crate will own write-then-verify-readback for every shard write (since
//! `fsync` over a network filesystem like SMB cannot be trusted), and will
//! expose the permission/ownership monitoring hook that `itsanas-repair`
//! uses to mark a node "degraded".
//!
//! Placeholder crate: no implementation yet. Real work starts at M3.
