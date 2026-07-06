#!/usr/bin/env bash
# Cross-compiles release binaries for every supported target (Standard B3):
# Linux x86_64, Linux ARM64 (Raspberry Pi 4B+), and Windows.
#
# Requires linkers for the non-native targets to be installed already:
#   - aarch64-unknown-linux-gnu: gcc-aarch64-linux-gnu (Debian/Ubuntu)
#   - x86_64-pc-windows-gnu:     mingw-w64 (Debian/Ubuntu)
# This script only orchestrates cargo; it does not install a cross-compiler
# toolchain, so switching CI providers never requires reworking this list.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

TARGETS=(
    "x86_64-unknown-linux-gnu"
    "aarch64-unknown-linux-gnu"
    "x86_64-pc-windows-gnu"
)

# itsanas-daemon is headless and runs everywhere, including the Raspberry Pi
# NAS box itself. itsanas-gui is the desktop companion app (Windows/Linux
# desktop) and isn't built for aarch64 — a Pi-based NAS has no screen to
# put it on.
package_binaries_for() {
    local target="$1"
    local bins=("itsanas-daemon")
    if [[ "${target}" != aarch64-* ]]; then
        bins+=("itsanas-gui")
    fi

    local out_dir="dist/${target}"
    mkdir -p "${out_dir}"
    for bin in "${bins[@]}"; do
        local src="target/${target}/release/${bin}"
        [[ "${target}" == *windows* ]] && src="${src}.exe"
        if [ -f "${src}" ]; then
            cp "${src}" "${out_dir}/"
        fi
    done
}

rm -rf dist
mkdir -p dist

for target in "${TARGETS[@]}"; do
    echo "==> adding rustup target ${target}"
    rustup target add "${target}"

    echo "==> building for ${target}"
    if [[ "${target}" == aarch64-* ]]; then
        cargo build --workspace --exclude itsanas-gui --release --target "${target}"
    else
        cargo build --workspace --release --target "${target}"
    fi

    package_binaries_for "${target}"
done

echo "==> release artifacts in dist/"
