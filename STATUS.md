# Status

Read this first when resuming work cold.

## Current milestone: M0 — crypto + chunking library

**In progress.**

### Done
- Repo governance: `LICENSE` (AGPL-3.0), `README.md`, `ARCHITECTURE.md`,
  `CONTRIBUTING.md`, `CODEOWNERS`, PR template, CLA workflow config.
- Cargo workspace scaffolded with `itsanas-crypto`, `itsanas-chunking` (real
  code) and placeholder crates for `itsanas-storage`, `itsanas-net`,
  `itsanas-repair`, `itsanas-quota`, `itsanas-daemon`, `itsanas-cli`.
- `itsanas-crypto`: Argon2id KDF, XChaCha20-Poly1305 encrypt/decrypt, Ed25519
  + X25519 identity keypair generation, key wrapping. Unit tests including
  known-answer and tamper-detection cases.
- `itsanas-chunking`: fixed-size chunking, BLAKE3 content addressing,
  verify-on-read. Unit tests including a corruption-detection case.
- `scripts/ci.sh` (fmt check, clippy, build, test) and `scripts/release.sh`
  (cross-compile), with `.github/workflows/ci.yml` as a thin wrapper that
  only calls the scripts (Standard/D11: CI provider never holds build logic).

### Not done yet (explicitly deferred)
- Branch protection on `main` — this needs to be turned on once this scaffold
  PR is reviewed and merged (see note below).
- CLA Assistant app — config file is in place
  (`.github/workflows/cla.yml` — actually a stub pointing at the CLA
  Assistant GitHub App); the app itself still needs to be **installed** by
  the repo owner (a one-click GitHub App install, not something Claude Code
  can do). Link is in the PR/handoff message.
- Everything past M0: storage, net (iroh), repair/scrubbing, quotas, daemon,
  CLI, Android — all placeholder crates only.

### Decisions applied this milestone
- D2 (AGPL-3.0 + CLA): license text sourced from the `spdx-license-list` npm
  package's bundled AGPL-3.0 full text (verified against the canonical GNU
  text) rather than typed out by the model, after the initial repo-init
  LICENSE turned out to be plain GPL-3.0 and was deleted by the owner.
- D10 crypto stack and D7 content-addressing implemented as specified.
- Fixed-size chunking chosen over CDC for M0 — see `ARCHITECTURE.md` for the
  rationale and the upgrade path.

## Next steps (M1)

1. Get this M0 scaffold reviewed and merged via PR.
2. Turn on branch protection on `main` (require PR + 1 owner review + green
   CI, no force pushes) — deferred until after this first merge since the
   session's push target is a feature branch, not direct-to-main.
3. Install the CLA Assistant GitHub App (owner action).
4. Begin M1: two-node LAN store/retrieve using `itsanas-storage` +
   `itsanas-net` (iroh) on top of the M0 crypto/chunking primitives.
