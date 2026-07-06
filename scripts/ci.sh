#!/usr/bin/env bash
# Runs everything CI runs: fmt check, clippy, build, test, fault-injection
# receipt, and a real end-to-end smoke test against running daemon
# instances (scripts/smoke-e2e.sh) — not just unit tests in isolation.
#
# This script is the actual CI logic (D11): the GitHub Actions workflow only
# calls this script, so switching CI providers is a matter of writing a new
# thin wrapper, not rewriting build/test logic. Run it locally before
# opening a PR — it's exactly what CI will run.
#
# Pass --full to additionally run everything that needs tools beyond the
# base Rust toolchain (curl/jq, which are ubiquitous, are always in
# scope): the Android network-contract logic tests (needs `gradle` with
# access to Maven Central) and the Windows installer build (needs
# mingw-w64 + nsis). These are skipped by default so `scripts/ci.sh` stays
# fast and portable (D11) — `--full` is "run literally everything,"
# meant for a real pre-merge check or a dedicated CI job, not every
# `cargo check`-speed iteration.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

FULL=0
for arg in "$@"; do
    case "$arg" in
    --full) FULL=1 ;;
    *)
        echo "unknown argument: $arg (only --full is supported)" >&2
        exit 2
        ;;
    esac
done

echo "==> cargo fmt --check"
cargo fmt --all -- --check

echo "==> cargo clippy"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> cargo build"
cargo build --workspace --all-targets

echo "==> cargo test"
cargo test --workspace

echo "==> receipt (fault-injection test mode)"
./scripts/receipt.sh

echo "==> smoke-e2e (real daemon instances, HTTP + on-disk assertions)"
./scripts/smoke-e2e.sh

if [ "$FULL" -eq 1 ]; then
    if command -v gradle >/dev/null 2>&1; then
        echo "==> android logic tests (network contract layer, no Android SDK needed)"
        ./scripts/test-android-logic.sh
    else
        echo "==> skipping android logic tests: gradle not found" >&2
    fi

    if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1 && command -v makensis >/dev/null 2>&1; then
        echo "==> windows installer build"
        ./scripts/package-windows-installer.sh
    else
        echo "==> skipping windows installer build: mingw-w64 and/or nsis not found" >&2
    fi

    if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1 && command -v wine >/dev/null 2>&1; then
        echo "==> windows tests under wine (fs-heavy crates as real Windows binaries)"
        ./scripts/test-windows.sh

        echo "==> windows e2e smoke test under wine (release daemon binary)"
        ./scripts/smoke-e2e-windows.sh
    else
        echo "==> skipping windows tests: mingw-w64 and/or wine not found" >&2
    fi
fi

echo "==> all checks passed"
