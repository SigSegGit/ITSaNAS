#!/usr/bin/env bash
# Receipt mode (Standard B4's simulation-mode idea, made concrete): proves
# each fault point is handled correctly, not just that the happy path
# works.
#
# Runs the M1 two-node LAN scenario (`itsanas-receipt`'s receipt-runner)
# once per known fault point (forcing that specific failure via
# ITSANAS_FAULT_POINT), then once more with no fault forced at all. Every
# run must produce the outcome the runner itself expects for that point —
# see itsanas-receipt/src/main.rs's `expected_for`. The fault point list is
# discovered from the binary itself (`--list-fault-points`), not
# hardcoded here, so adding a FaultPoint in itsanas-testkit is enough for
# this script to pick it up.
#
# Writes receipt.md summarizing every run, and exits non-zero if any run's
# outcome didn't match what was expected.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

echo "==> building receipt-runner"
cargo build --quiet -p itsanas-receipt

RUNNER="target/debug/receipt-runner"
RECEIPT_FILE="receipt.md"
FAILED=0

{
    echo "# ITSaNAS Receipt"
    echo
    echo "- commit: $(git rev-parse HEAD)"
    echo "- date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo
} >"$RECEIPT_FILE"

mapfile -t FAULT_POINTS < <("$RUNNER" --list-fault-points)
if [ "${#FAULT_POINTS[@]}" -eq 0 ]; then
    echo "==> no fault points reported by receipt-runner; nothing to check" >&2
    exit 1
fi

for point in "${FAULT_POINTS[@]}"; do
    echo "==> fault point: $point"
    if ITSANAS_FAULT_POINT="$point" "$RUNNER"; then
        echo "- [x] \`$point\`: handled correctly" >>"$RECEIPT_FILE"
    else
        echo "- [ ] \`$point\`: NOT handled correctly" >>"$RECEIPT_FILE"
        FAILED=1
    fi
done

echo "==> clean run (no fault forced)"
if "$RUNNER"; then
    echo "- [x] clean run: succeeded" >>"$RECEIPT_FILE"
else
    echo "- [ ] clean run: FAILED" >>"$RECEIPT_FILE"
    FAILED=1
fi

echo
cat "$RECEIPT_FILE"

if [ "$FAILED" -ne 0 ]; then
    echo "==> receipt FAILED — see $RECEIPT_FILE" >&2
    exit 1
fi
echo "==> receipt OK"
