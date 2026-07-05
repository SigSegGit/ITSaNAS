//! Receipt-runner: runs the M1+M3 scenario once, either cleanly or under
//! one forced [`FaultPoint`], and reports (via exit code and a one-line
//! message) whether the outcome matches what's expected.
//!
//! `scripts/receipt.sh` discovers the fault point list from
//! `--list-fault-points` rather than hardcoding it, runs this binary once
//! per point (via `ITSANAS_FAULT_POINT`) plus once with no fault point set
//! (the clean run), and fails loudly if any run's outcome doesn't match.

mod scenario;

use itsanas_net::NetError;
use itsanas_repair::RepairError;
use itsanas_testkit::FaultPoint;
use scenario::ScenarioError;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--list-fault-points") {
        for point in FaultPoint::ALL {
            println!("{}", point.name());
        }
        return;
    }

    let requested = std::env::var("ITSANAS_FAULT_POINT")
        .ok()
        .and_then(|name| FaultPoint::parse(&name));

    let rt = tokio::runtime::Runtime::new().expect("failed to start tokio runtime");
    let result = rt.block_on(scenario::run());

    std::process::exit(report(requested, result));
}

/// Decides pass/fail, prints a one-line verdict, and returns the process
/// exit code: 0 for "behaved exactly as expected," 1 otherwise.
fn report(requested: Option<FaultPoint>, result: Result<(), ScenarioError>) -> i32 {
    match (requested, result) {
        (None, Ok(())) => {
            println!("clean run: OK");
            0
        }
        (None, Err(e)) => {
            eprintln!("clean run: FAILED unexpectedly: {e}");
            1
        }
        (Some(point), Ok(())) => {
            eprintln!(
                "{}: requested but had NO observable effect \
                 (fault injection not wired up for this point?)",
                point.name()
            );
            1
        }
        (Some(point), Err(e)) => {
            if expected_for(point, &e) {
                println!("{}: handled correctly ({e})", point.name());
                0
            } else {
                eprintln!("{}: failed, but not in the expected way: {e}", point.name());
                1
            }
        }
    }
}

/// What kind of [`ScenarioError`] each fault point is expected to surface
/// as, when the surrounding code detects and handles it correctly.
fn expected_for(point: FaultPoint, error: &ScenarioError) -> bool {
    match (point, error) {
        // The server's own storage.put() catches the corruption via
        // write-then-verify-readback and reports it back as a remote error.
        (FaultPoint::StorageWriteCorruption, ScenarioError::Net(NetError::Remote(_))) => true,
        // The server's storage.get() fails and reports it back as a remote error.
        (FaultPoint::StorageGetIoFailure, ScenarioError::Net(NetError::Remote(_))) => true,
        // The requester re-verifies content on receipt and catches the tamper itself.
        (FaultPoint::NetShardTamperInTransit, ScenarioError::Net(NetError::Verify(_))) => true,
        // The connection drops before a response ever arrives.
        (FaultPoint::NetPeerDisconnectMidTransfer, ScenarioError::Net(NetError::Transport(_))) => {
            true
        }
        // repair() tries every mirror, finds none reachable, and reports
        // a clean aggregate failure rather than panicking or hanging.
        (
            FaultPoint::RepairAllMirrorsUnreachable,
            ScenarioError::Repair(RepairError::NoHealthyMirror(_)),
        ) => true,
        _ => false,
    }
}
