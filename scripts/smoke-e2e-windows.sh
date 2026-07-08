#!/usr/bin/env bash
# Runs the full smoke-e2e suite (scripts/smoke-e2e.sh — real daemons,
# HTTP + on-disk assertions) against the *Windows* daemon binary under
# Wine. Same assertions, different operating system underneath.
#
# This exists because scripts/test-windows.sh only covers the fs-heavy
# crates' unit tests; the daemon itself (HTTP server, sync engine, vault
# lifecycle, background scrub) is a different program on Windows —
# different filesystem semantics, different socket stack — and "the
# installer runs on a machine we've never tested on" is exactly how the
# os-error-5 shard-write bug shipped.
#
# Requires: mingw-w64 and wine. scripts/ci.sh --full auto-skips when
# either is missing.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

TARGET="x86_64-pc-windows-gnu"
rustup target add "${TARGET}"

# Wine translates Unix filesystem paths to/from Windows UTF-16 using the
# process locale's charset. Under a non-UTF-8 locale that translation
# silently mangles non-ASCII names — this is what turned "an accented
# file in the synced folder" into a mystery failure the first time this
# suite ran here, purely because the container's default locale is
# POSIX/C, not because of any bug in the daemon. Force UTF-8 so this
# suite's result reflects the code, not the host's env.
export LANG=C.UTF-8
export LC_ALL=C.UTF-8

# Release, not debug: (a) it's the same profile the shipped installer
# binary is built with, so it's the thing actually worth testing, and
# (b) debug windows-gnu builds of the iroh-relay dependency fail to link
# outright ("export ordinal too large" — mingw's 64k DLL-export limit,
# which release symbol GC stays under).
echo "==> building itsanas-daemon (release) for ${TARGET}"
cargo build --quiet --release -p itsanas-daemon --target "${TARGET}"

wineboot --init >/dev/null 2>&1 || true

# Ubuntu's wine 9.0 packaging bug: wine's own PE builds of user32/shell32/
# crypt32 (which the daemon pulls in via cert-store and known-folder APIs)
# depend on zlib1.dll, which ships in wine's package directory but isn't
# found by the loader. Without this copy the daemon dies on startup with
# a cascade of "Library zlib1.dll not found" import errors.
WINE_DLL_DIR="/usr/lib/x86_64-linux-gnu/wine/x86_64-windows"
PREFIX_SYS32="${WINEPREFIX:-$HOME/.wine}/drive_c/windows/system32"
if [ -f "${WINE_DLL_DIR}/zlib1.dll" ] && [ ! -f "${PREFIX_SYS32}/zlib1.dll" ]; then
    cp "${WINE_DLL_DIR}/zlib1.dll" "${PREFIX_SYS32}/"
fi

DAEMON_CMD="wine ./target/${TARGET}/release/itsanas-daemon.exe" \
    exec ./scripts/smoke-e2e.sh
