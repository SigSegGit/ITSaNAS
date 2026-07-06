#!/usr/bin/env bash
# End-to-end smoke test against real running itsanas-daemon instances —
# the same checks performed manually during development (see TESTING.md),
# turned into a permanent, repeatable, automatically-asserted script so
# they can never silently regress.
#
# Unlike scripts/ci.sh (fmt/clippy/build/unit-test/receipt, which never
# touches a real running process), this actually starts daemons, talks to
# them over HTTP, and inspects what lands on disk. Covers:
#   - account setup/unlock/lock and the password<->vault binding
#   - two-account isolation (wrong password can't unlock another vault)
#   - "stolen data directory" (full on-disk copy still needs the password)
#   - at-rest encryption, including that file names don't leak anywhere
#     on disk (manifest, sync state, or otherwise)
#   - large binary file round-trips byte-for-byte through the HTTP API
#   - folder-sync engine, both directions, including delete propagation
#   - locked vault blocks sync and API access alike
#
# Every check is asserted, not eyeballed: failures are collected and
# reported at the end (so one broken thing doesn't hide the next), and
# the script exits non-zero if anything failed. Safe to re-run — uses
# fresh temp directories and high, unlikely-to-collide ports every time.

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

ROOT="$(mktemp -d)"
FAILURES=0
PIDS=()

cleanup() {
    for pid in "${PIDS[@]:-}"; do
        kill -9 "$pid" >/dev/null 2>&1 || true
    done
    rm -rf "$ROOT"
}
trap cleanup EXIT

fail() {
    echo "  FAIL: $1" >&2
    FAILURES=$((FAILURES + 1))
}

pass() {
    echo "  ok: $1"
}

assert_eq() {
    local actual="$1" expected="$2" desc="$3"
    if [ "$actual" = "$expected" ]; then
        pass "$desc"
    else
        fail "$desc (expected [$expected], got [$actual])"
    fi
}

assert_contains() {
    local haystack="$1" needle="$2" desc="$3"
    if grep -qF -- "$needle" <<<"$haystack"; then
        pass "$desc"
    else
        fail "$desc (did not find [$needle])"
    fi
}

assert_not_in_file() {
    local file="$1" needle="$2" desc="$3"
    if [ -f "$file" ] && grep -qF -- "$needle" "$file" 2>/dev/null; then
        fail "$desc (found [$needle] in $file — should be encrypted/absent)"
    else
        pass "$desc"
    fi
}

assert_not_in_dir() {
    local dir="$1" needle="$2" desc="$3"
    if grep -rqF -- "$needle" "$dir" 2>/dev/null; then
        fail "$desc (found [$needle] somewhere under $dir — should be encrypted/absent)"
    else
        pass "$desc"
    fi
}

# Starts a daemon on $1 (port) with data dir $2 and sync dir $3, recording
# its PID for cleanup, and blocks until it actually answers /status (so
# every later request in this script can assume the daemon is up, rather
# than racing a fixed sleep).
start_daemon() {
    local port="$1" data_dir="$2" sync_dir="$3" log="$4"
    mkdir -p "$data_dir" "$sync_dir"
    ITSANAS_DATA_DIR="$data_dir" ITSANAS_SYNC_DIR="$sync_dir" ITSANAS_PORT="$port" \
        ./target/debug/itsanas-daemon >"$log" 2>&1 &
    PIDS+=("$!")

    local waited=0
    until curl -s -o /dev/null "http://127.0.0.1:$port/status"; do
        sleep 0.2
        waited=$((waited + 1))
        if [ "$waited" -gt 50 ]; then
            fail "daemon on port $port never came up (see $log)"
            return 1
        fi
    done
}

# Polls until $2 (a jq filter over `curl $1`) prints "true", or times out.
wait_until() {
    local url="$1" jq_filter="$2" desc="$3"
    local waited=0
    until [ "$(curl -s "$url" | jq -r "$jq_filter" 2>/dev/null)" = "true" ]; do
        sleep 0.3
        waited=$((waited + 1))
        if [ "$waited" -gt 30 ]; then
            fail "$desc (timed out waiting on $url)"
            return 1
        fi
    done
    pass "$desc"
}

echo "==> building itsanas-daemon"
cargo build --quiet -p itsanas-daemon

# A short scrub interval so the background-scrub check below doesn't have
# to wait out the real (multi-hour) default.
export ITSANAS_SCRUB_INTERVAL_SECS=1

PORT_A=14279
PORT_B=14280
PORT_STOLEN=14281
DIR_A="$ROOT/alice"
DIR_B="$ROOT/bob"
DIR_STOLEN="$ROOT/stolen"

echo "==> starting two independent daemon instances (alice, bob)"
start_daemon "$PORT_A" "$DIR_A/data" "$DIR_A/synced" "$ROOT/alice.log"
start_daemon "$PORT_B" "$DIR_B/data" "$DIR_B/synced" "$ROOT/bob.log"

echo "==> account setup"
code=$(curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$PORT_A/account/setup" \
    -H 'Content-Type: application/json' -d '{"password":"alice-secret-pw-1"}')
assert_eq "$code" "201" "alice account setup succeeds"

code=$(curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$PORT_B/account/setup" \
    -H 'Content-Type: application/json' -d '{"password":"bob-different-pw-2"}')
assert_eq "$code" "201" "bob account setup succeeds"

echo "==> per-account file isolation"
curl -s -X PUT "http://127.0.0.1:$PORT_A/files/alices-diary.txt" \
    --data-binary 'ALICE_SECRET_MARKER: private diary contents' >/dev/null
curl -s -X PUT "http://127.0.0.1:$PORT_B/files/bobs-notes.txt" \
    --data-binary 'BOB_SECRET_MARKER: private notes' >/dev/null

alice_files=$(curl -s "http://127.0.0.1:$PORT_A/files")
assert_contains "$alice_files" "alices-diary.txt" "alice sees her own file"
assert_eq "$(jq 'length' <<<"$alice_files")" "1" "alice sees exactly one file (not bob's)"

bob_files=$(curl -s "http://127.0.0.1:$PORT_B/files")
assert_contains "$bob_files" "bobs-notes.txt" "bob sees his own file"
assert_eq "$(jq 'length' <<<"$bob_files")" "1" "bob sees exactly one file (not alice's)"

echo "==> wrong password cannot unlock another account's vault"
curl -s -X POST "http://127.0.0.1:$PORT_B/account/lock" >/dev/null
code=$(curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$PORT_B/account/unlock" \
    -H 'Content-Type: application/json' -d '{"password":"alice-secret-pw-1"}')
assert_eq "$code" "401" "bob's vault rejects alice's password"

code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$PORT_B/files")
assert_eq "$code" "401" "bob's vault stays locked after the failed unlock"

code=$(curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$PORT_B/account/unlock" \
    -H 'Content-Type: application/json' -d '{"password":"bob-different-pw-2"}')
assert_eq "$code" "200" "bob's own password unlocks correctly"

echo "==> at-rest encryption: no plaintext file names or content anywhere on disk"
assert_not_in_dir "$DIR_A/data" "ALICE_SECRET_MARKER" "alice's data dir never contains her file's plaintext content"
assert_not_in_dir "$DIR_A/data" "alices-diary" "alice's data dir never contains her file's plaintext name"

echo "==> stolen vault data is useless without the exact password"
mkdir -p "$DIR_STOLEN"
cp -r "$DIR_A/data" "$DIR_STOLEN/data"
start_daemon "$PORT_STOLEN" "$DIR_STOLEN/data" "$DIR_STOLEN/synced" "$ROOT/stolen.log"

code=$(curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$PORT_STOLEN/account/unlock" \
    -H 'Content-Type: application/json' -d '{"password":"a-random-guess"}')
assert_eq "$code" "401" "a guessed password fails against the stolen copy"

code=$(curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$PORT_STOLEN/account/unlock" \
    -H 'Content-Type: application/json' -d '{"password":"bob-different-pw-2"}')
assert_eq "$code" "401" "a different account's real password also fails against the stolen copy"

code=$(curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$PORT_STOLEN/account/unlock" \
    -H 'Content-Type: application/json' -d '{"password":"alice-secret-pw-1"}')
assert_eq "$code" "200" "only the real original password unlocks the stolen copy"

echo "==> large binary file round-trips byte-for-byte"
BIGFILE="$ROOT/random_3mb.bin"
head -c 3145728 /dev/urandom >"$BIGFILE"
expected_hash=$(sha256sum "$BIGFILE" | awk '{print $1}')

code=$(curl -s -o /dev/null -w '%{http_code}' -X PUT "http://127.0.0.1:$PORT_A/files/random_3mb.bin" --data-binary "@$BIGFILE")
assert_eq "$code" "201" "3 MiB binary file uploads successfully (no body-size limit surprises)"

curl -s "http://127.0.0.1:$PORT_A/files/random_3mb.bin" -o "$ROOT/downloaded_3mb.bin"
actual_hash=$(sha256sum "$ROOT/downloaded_3mb.bin" | awk '{print $1}')
assert_eq "$actual_hash" "$expected_hash" "downloaded file is byte-for-byte identical (sha256 match)"

echo "==> background scrub detects a corrupted file and reports it by name (D7)"
wait_until "http://127.0.0.1:$PORT_A/status" '.vault_health != null' \
    "the first background scrub completes and populates vault_health"

before_shards=$(find "$DIR_A/data/shards" -type f | sort)
curl -s -X PUT "http://127.0.0.1:$PORT_A/files/for-scrub-test.txt" \
    --data-binary 'content that will get corrupted for the scrub test' >/dev/null
after_shards=$(find "$DIR_A/data/shards" -type f | sort)
new_shard=$(comm -13 <(echo "$before_shards") <(echo "$after_shards") | head -1)

if [ -z "$new_shard" ]; then
    fail "background scrub test: could not identify the new shard file to corrupt"
else
    # Corrupt it directly on disk, bypassing put()'s own
    # write-then-verify-readback (which would just reject it at write time).
    echo "corrupted content for scrub test" >"$new_shard"
    wait_until "http://127.0.0.1:$PORT_A/status" \
        '.vault_health.unhealthy_files | contains(["for-scrub-test.txt"])' \
        "background scrub detects the corrupted file and reports it by name via /status"
fi

echo "==> folder sync: folder -> vault"
echo "hello from the synced folder" >"$DIR_A/synced/from-folder.txt"
wait_until "http://127.0.0.1:$PORT_A/files" '[.[] | .name] | contains(["from-folder.txt"])' \
    "a file dropped into the synced folder appears in the vault"

echo "==> folder sync: vault -> folder"
curl -s -X PUT "http://127.0.0.1:$PORT_A/files/from-api.txt" --data-binary 'uploaded via the API' >/dev/null
waited=0
until [ -f "$DIR_A/synced/from-api.txt" ]; do
    sleep 0.3
    waited=$((waited + 1))
    if [ "$waited" -gt 30 ]; then
        fail "a file PUT via the API materializes in the synced folder (timed out)"
        break
    fi
done
[ -f "$DIR_A/synced/from-api.txt" ] && pass "a file PUT via the API materializes in the synced folder"

echo "==> folder sync: deletes propagate both directions"
rm -f "$DIR_A/synced/from-folder.txt"
wait_until "http://127.0.0.1:$PORT_A/files" '[.[] | .name] | contains(["from-folder.txt"]) | not' \
    "deleting a file from the synced folder removes it from the vault"

curl -s -X DELETE "http://127.0.0.1:$PORT_A/files/from-api.txt" >/dev/null
waited=0
while [ -f "$DIR_A/synced/from-api.txt" ]; do
    sleep 0.3
    waited=$((waited + 1))
    if [ "$waited" -gt 30 ]; then
        fail "deleting a file via the API removes it from the synced folder (timed out)"
        break
    fi
done
[ ! -f "$DIR_A/synced/from-api.txt" ] && pass "deleting a file via the API removes it from the synced folder"

echo "==> a locked vault blocks both the API and folder sync"
curl -s -X POST "http://127.0.0.1:$PORT_A/account/lock" >/dev/null
echo "should not sync while locked" >"$DIR_A/synced/locked-test.txt"
sleep 1
code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$PORT_A/files")
assert_eq "$code" "401" "the API is locked out after /account/lock"

vault_health=$(curl -s "http://127.0.0.1:$PORT_A/status" | jq -c '.vault_health')
assert_eq "$vault_health" "null" "a locked vault hides its health report too (reveals nothing, not even that)"

curl -s -X POST "http://127.0.0.1:$PORT_A/account/unlock" \
    -H 'Content-Type: application/json' -d '{"password":"alice-secret-pw-1"}' >/dev/null
wait_until "http://127.0.0.1:$PORT_A/files" '[.[] | .name] | contains(["locked-test.txt"])' \
    "the file written while locked syncs in once unlocked again"

echo
if [ "$FAILURES" -eq 0 ]; then
    echo "==> smoke-e2e OK (all checks passed)"
    exit 0
else
    echo "==> smoke-e2e FAILED: $FAILURES check(s) failed" >&2
    exit 1
fi
