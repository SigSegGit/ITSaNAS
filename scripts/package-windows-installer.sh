#!/usr/bin/env bash
# Builds the single-click Windows installer (packaging/windows/installer.nsi)
# for itsanas-daemon + itsanas-gui.
#
# Requires (Debian/Ubuntu):
#   - mingw-w64 (cross-compiler for x86_64-pc-windows-gnu)
#   - nsis (provides `makensis`)

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

TARGET="x86_64-pc-windows-gnu"

echo "==> adding rustup target ${TARGET}"
rustup target add "${TARGET}"

echo "==> building itsanas-daemon and itsanas-gui for ${TARGET}"
cargo build --release --target "${TARGET}" -p itsanas-daemon -p itsanas-gui

mkdir -p dist
echo "==> running makensis"
makensis packaging/windows/installer.nsi

echo "==> installer written to dist/itsanas-installer.exe"
