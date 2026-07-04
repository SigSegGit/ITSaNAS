//! Fault-injection registry for ITSaNAS's test mode.
//!
//! Other crates depend on this **only** behind an optional `test-mode`
//! Cargo feature (see `itsanas-storage` and `itsanas-net`). A production
//! release build never enables that feature, so this crate — and every
//! fault-injection call site that uses it — is compiled out of production
//! binaries entirely, not just disabled at runtime. That's a deliberate
//! safety property: there is no code path by which fault injection could
//! ever activate outside a deliberately-built test/receipt binary.
//!
//! [`FaultPoint`] is the single source of truth for "what can we force to
//! fail on purpose." `scripts/receipt.sh` discovers the full list from the
//! running binary itself (`FaultPoint::ALL`) rather than hardcoding it, so
//! adding a fault point here is enough to have the receipt script pick it
//! up automatically.

/// A single point in the system where a specific failure can be forced,
/// to prove the surrounding code detects and handles it correctly.
///
/// Add a new variant here, add it to [`FaultPoint::ALL`], and call
/// [`should_fail`] at the point in the code being tested — see
/// `itsanas-storage`'s and `itsanas-net`'s `test-mode` call sites for the
/// existing examples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FaultPoint {
    /// A shard's bytes get corrupted on disk during
    /// `StorageRoot::put`'s write, before the write-then-verify-readback
    /// check runs. Proves D7's write-then-verify-readback actually catches
    /// on-write corruption rather than trusting `fsync`.
    StorageWriteCorruption,
    /// `StorageRoot::get` fails as if the storage backend refused to read
    /// a shard that is actually present. Proves storage failures propagate
    /// as a clean error across the network rather than panicking or
    /// hanging the connection.
    StorageGetIoFailure,
    /// A shard's bytes get corrupted in transit, after being read from
    /// storage but before being sent to the requesting peer. Proves the
    /// client re-verifies content on receipt (D7: the network is not a
    /// trust boundary either).
    NetShardTamperInTransit,
    /// The serving peer drops the connection instead of responding.
    /// Proves the requesting peer surfaces this as a transport error
    /// rather than hanging indefinitely.
    NetPeerDisconnectMidTransfer,
}

impl FaultPoint {
    /// Every known fault point, in stable order. `scripts/receipt.sh`
    /// drives this list via the receipt-runner's `--list-fault-points`.
    pub const ALL: &'static [FaultPoint] = &[
        FaultPoint::StorageWriteCorruption,
        FaultPoint::StorageGetIoFailure,
        FaultPoint::NetShardTamperInTransit,
        FaultPoint::NetPeerDisconnectMidTransfer,
    ];

    /// The stable string form used in `ITSANAS_FAULT_POINT` and
    /// `--list-fault-points` output.
    pub fn name(self) -> &'static str {
        match self {
            FaultPoint::StorageWriteCorruption => "storage-write-corruption",
            FaultPoint::StorageGetIoFailure => "storage-get-io-failure",
            FaultPoint::NetShardTamperInTransit => "net-shard-tamper-in-transit",
            FaultPoint::NetPeerDisconnectMidTransfer => "net-peer-disconnect-mid-transfer",
        }
    }

    /// Parses a fault point back from its [`FaultPoint::name`].
    pub fn parse(name: &str) -> Option<FaultPoint> {
        Self::ALL.iter().copied().find(|p| p.name() == name)
    }
}

/// Whether `point` should be made to fail right now, given `active` as the
/// currently-requested fault point (or `None` for a clean run).
///
/// Pure and side-effect-free so it's trivially unit-testable without
/// touching real environment variables (which would be racy under
/// parallel test execution).
pub fn should_fail_given(active: Option<FaultPoint>, point: FaultPoint) -> bool {
    active == Some(point)
}

/// Reads the currently-requested fault point from the `ITSANAS_FAULT_POINT`
/// environment variable, if any. Re-reads on every call rather than
/// caching — call sites are rare enough that this costs nothing, and it
/// keeps behavior simple to reason about.
pub fn active_from_env() -> Option<FaultPoint> {
    std::env::var("ITSANAS_FAULT_POINT")
        .ok()
        .and_then(|name| FaultPoint::parse(&name))
}

/// Whether `point` should be made to fail right now, per the
/// `ITSANAS_FAULT_POINT` environment variable.
pub fn should_fail(point: FaultPoint) -> bool {
    should_fail_given(active_from_env(), point)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fault_point_name_parses_back_to_itself() {
        for point in FaultPoint::ALL {
            assert_eq!(FaultPoint::parse(point.name()), Some(*point));
        }
    }

    #[test]
    fn unknown_name_does_not_parse() {
        assert_eq!(FaultPoint::parse("no-such-fault-point"), None);
    }

    #[test]
    fn names_are_pairwise_distinct() {
        let mut names: Vec<&str> = FaultPoint::ALL.iter().map(|p| p.name()).collect();
        let original_len = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), original_len, "duplicate fault point names");
    }

    #[test]
    fn should_fail_given_matches_only_the_active_point() {
        let active = Some(FaultPoint::StorageWriteCorruption);
        assert!(should_fail_given(
            active,
            FaultPoint::StorageWriteCorruption
        ));
        assert!(!should_fail_given(active, FaultPoint::StorageGetIoFailure));
    }

    #[test]
    fn should_fail_given_with_no_active_point_never_fails() {
        for point in FaultPoint::ALL {
            assert!(!should_fail_given(None, *point));
        }
    }
}
