#!/usr/bin/env bash
# Runs the filesystem-heavy crates' test suites as real Windows binaries
# under Wine. Exists because Windows filesystem semantics genuinely
# differ from Linux in ways unit tests on Linux can't see: the first
# real Windows install failed every shard write with "Access denied"
# (os error 5) because FlushFileBuffers refuses read-only handles,
# while fsync on Linux happily accepts them. Under Wine, the old code
# fails 9 of 12 itsanas-storage tests; the fixed code passes all —
# i.e. this harness genuinely reproduces that class of bug.
#
# Scope: crates whose behavior is dominated by std::fs (storage,
# chunking, crypto). The daemon/GUI pull in tokio/iroh networking,
# which Wine doesn't emulate faithfully enough for signal over noise.
#
# Requires: mingw-w64 (cross-compiler) and wine. scripts/ci.sh --full
# auto-skips this script when either is missing.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

TARGET="x86_64-pc-windows-gnu"

rustup target add "${TARGET}"

# Wine writes ~/.wine on first run; do it once up front so test output
# isn't interleaved with wineboot noise.
wineboot --init >/dev/null 2>&1 || true

export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=wine
cargo test --target "${TARGET}" \
    -p itsanas-storage \
    -p itsanas-chunking \
    -p itsanas-crypto

echo "windows-tests: OK"
