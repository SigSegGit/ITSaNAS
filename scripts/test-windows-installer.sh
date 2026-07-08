#!/usr/bin/env bash
# Silent-installs the real NSIS installer under Wine and asserts what it
# actually produces: the two binaries on disk, the uninstall/autostart
# registry entries, and — the part that actually matters — that the
# installed itsanas-daemon.exe runs and answers HTTP. Building the
# installer proves makensis accepts the script; nothing before this ran
# what it installs, which is exactly the gap that let a real Windows
# install fail on day one.
#
# Requires: mingw-w64, nsis (makensis), and a wine64-capable Wine (the
# plain Ubuntu `wine` package's default prefix is 32-bit-only; the
# `wine64` package supplies the win64 loader needed for the 64-bit
# itsanas-*.exe binaries). scripts/ci.sh --full auto-skips this script
# when the tools aren't present.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

export LANG=C.UTF-8
export LC_ALL=C.UTF-8

echo "==> building the installer"
./scripts/package-windows-installer.sh >/dev/null

# A fresh win64 prefix every run: reusing a prefix that was ever
# WINEARCH=win32 (Ubuntu's default for a from-scratch `wineboot --init`
# without an explicit win64 loader) can't run a 64-bit PE at all and
# fails with "Bad EXE format" — found the hard way tonight. wine64
# (distinct from the base `wine` package) provides the win64 loader.
WINEPREFIX="$(mktemp -d)"
export WINEPREFIX
trap 'rm -rf "$WINEPREFIX"' EXIT

echo "==> creating a fresh win64 Wine prefix"
WINEARCH=win64 /usr/lib/wine/wine64 wineboot --init >/dev/null 2>&1 || true

echo "==> silently installing itsanas-installer.exe"
wine dist/itsanas-installer.exe /S
sleep 2

FAILURES=0
fail() { echo "  FAIL: $1" >&2; FAILURES=$((FAILURES + 1)); }
pass() { echo "  ok: $1"; }

INSTALL_DIR="${WINEPREFIX}/drive_c/users/$(whoami)/AppData/Local/Programs/ITSaNAS"

for f in itsanas-daemon.exe itsanas-gui.exe uninstall.exe; do
    if [ -f "${INSTALL_DIR}/${f}" ]; then
        pass "${f} is installed"
    else
        fail "${f} was not installed"
    fi
done

if wine reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v ITSaNAS \
    >/dev/null 2>&1; then
    pass "the login-autostart registry entry was written"
else
    fail "the login-autostart registry entry is missing"
fi

if wine reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\ITSaNAS" \
    >/dev/null 2>&1; then
    pass "the uninstall registry entry was written"
else
    fail "the uninstall registry entry is missing"
fi

echo "==> starting the installed daemon and checking it actually answers HTTP"
DATA_DIR="$(mktemp -d)"
SYNC_DIR="$(mktemp -d)"
PORT=14915
ITSANAS_DATA_DIR="$DATA_DIR" ITSANAS_SYNC_DIR="$SYNC_DIR" ITSANAS_PORT="$PORT" \
    wine "${INSTALL_DIR}/itsanas-daemon.exe" >"${WINEPREFIX}/daemon.log" 2>&1 &
DAEMON_PID=$!

waited=0
until curl -s -o /dev/null "http://127.0.0.1:${PORT}/status"; do
    sleep 0.3
    waited=$((waited + 1))
    if [ "$waited" -gt 40 ]; then
        fail "the installed daemon never answered /status (see ${WINEPREFIX}/daemon.log)"
        break
    fi
done
if curl -s "http://127.0.0.1:${PORT}/status" | grep -q '"has_account":false'; then
    pass "the installed daemon serves a well-formed /status response"
else
    fail "the installed daemon's /status response looked wrong"
fi

kill -9 "$DAEMON_PID" >/dev/null 2>&1 || true
rm -rf "$DATA_DIR" "$SYNC_DIR"

if [ "$FAILURES" -eq 0 ]; then
    echo "==> test-windows-installer OK (all checks passed)"
else
    echo "==> test-windows-installer FAILED: ${FAILURES} check(s) failed"
    exit 1
fi
