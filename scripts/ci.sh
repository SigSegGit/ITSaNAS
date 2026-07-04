#!/usr/bin/env bash
# Runs everything CI runs: fmt check, clippy, build, test.
#
# This script is the actual CI logic (D11): the GitHub Actions workflow only
# calls this script, so switching CI providers is a matter of writing a new
# thin wrapper, not rewriting build/test logic. Run it locally before
# opening a PR — it's exactly what CI will run.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

echo "==> cargo fmt --check"
cargo fmt --all -- --check

echo "==> cargo clippy"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> cargo build"
cargo build --workspace --all-targets

echo "==> cargo test"
cargo test --workspace

echo "==> all checks passed"
