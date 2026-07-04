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

# Binaries this workspace produces once their crates gain a `main.rs`
# (itsanas-cli, itsanas-daemon are placeholders as of M0 — this list will
# grow as those crates gain binary entry points).
BINARIES=()

rm -rf dist
mkdir -p dist

for target in "${TARGETS[@]}"; do
    echo "==> adding rustup target ${target}"
    rustup target add "${target}"

    echo "==> building for ${target}"
    cargo build --workspace --release --target "${target}"

    if [ "${#BINARIES[@]}" -eq 0 ]; then
        continue
    fi

    out_dir="dist/${target}"
    mkdir -p "${out_dir}"
    for bin in "${BINARIES[@]}"; do
        src="target/${target}/release/${bin}"
        [[ "${target}" == *windows* ]] && src="${src}.exe"
        if [ -f "${src}" ]; then
            cp "${src}" "${out_dir}/"
        fi
    done
done

echo "==> release artifacts in dist/ (empty until itsanas-cli/itsanas-daemon gain binaries)"
