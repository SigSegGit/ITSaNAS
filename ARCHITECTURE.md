# Architecture

This document maps the project's locked design decisions to the crates that
implement them, and records why each major dependency was chosen. Keep it
current: every new crate or dependency should be justified here.

## Workspace layout

```
crates/
  itsanas-crypto     encryption, key derivation, identities, key wrapping
  itsanas-chunking    content addressing, chunking, verify-on-read
  itsanas-storage    local storage root management, shard I/O (placeholder)
  itsanas-net        iroh transport, discovery, relay (placeholder)
  itsanas-repair     scrubbing, tamper/corruption detection, re-replication (placeholder)
  itsanas-quota      per-user contributed/usable space accounting (placeholder)
  itsanas-daemon     background service tying the above together (placeholder)
  itsanas-cli        command-line client (placeholder)
```

Each placeholder crate contains only a doc-header `lib.rs` and a `README.md`
describing its intended responsibility until its milestone is reached.

## Decision → crate map

| # | Decision | Implementing crate(s) |
|---|---|---|
| D1 | Project name / repo | — (governance) |
| D2 | AGPL-3.0 + CLA, owner retains relicensing rights | — (governance: `LICENSE`, `CONTRIBUTING.md`, CLA workflow) |
| D3 | Sole-validator governance, branch protection, CODEOWNERS | — (governance: `CODEOWNERS`, repo settings) |
| D4 | iroh P2P transport, self-hosted relay, never fall back to public iroh infra | `itsanas-net` |
| D5 | Freebox VM as relay/bootstrap/rendezvous, architected as "just another node"; must be removable before the 10th user | `itsanas-net` |
| D6 | Mirroring at N<4, Reed–Solomon erasure coding at N≥4, ≤3× contribution overhead | `itsanas-repair`, `itsanas-quota` |
| D7 | Hostile/unreliable storage backends: opaque ciphertext shards, BLAKE3 verify-on-read, scrubbing, permission-change detection, write-then-verify-readback | `itsanas-chunking` (content addressing), `itsanas-storage` (write-then-verify), `itsanas-repair` (scrubbing, monitoring) |
| D8 | BIP39 mnemonic recovery kit | `itsanas-crypto` |
| D9 | Android thin client over the daemon's authenticated API, min API 29 | `itsanas-daemon` (API), Android client (future, separate repo/module) |
| D10 | Crypto stack: XChaCha20-Poly1305, X25519/Ed25519, Argon2id, BLAKE3 | `itsanas-crypto` (cipher, identities, KDF), `itsanas-chunking` (content addressing) |
| D11 | No metered dependencies; portable scripts + thin CI wrapper | `scripts/ci.sh`, `scripts/release.sh`, `.github/workflows/ci.yml` |
| D12 | Open code, private networks (invite-only) | `itsanas-net` (invite/join flow, future), `README.md` |
| D13 | Connectivity self-test (CGNAT detection) on first run | `itsanas-net`, `itsanas-daemon` |

## `itsanas-crypto`

Implements decision D10 (cipher suite) and D8 (recovery). Responsibilities:

- **Key derivation**: Argon2id turns a user password + random salt into a
  32-byte master key. Parameters are deliberately explicit (not just
  defaults) so they can be tuned and are documented at the call site.
- **Symmetric encryption**: XChaCha20-Poly1305 (24-byte random nonce, AEAD)
  for both file content and key wrapping. XChaCha20's large nonce space
  makes random nonce generation safe without a counter, which matters for a
  P2P system with no central nonce coordinator.
- **Identities**: Ed25519 keypairs for signing (peer identity, message
  authentication) and X25519 keypairs for key exchange, generated from a
  CSPRNG.
- **Key wrapping**: per-file keys are generated randomly and wrapped
  (encrypted) with the user's master key, so the master key is never used
  directly to encrypt bulk data.

Recovery-kit (BIP39 mnemonic) generation is planned for M4 (accounts) and
will live in this crate; M0 only implements the primitives it depends on.

## `itsanas-chunking`

Implements the content-addressing half of D7 and D10.

**Chunking strategy (M0): fixed-size chunking.** Content-defined chunking
(CDC, e.g. FastCDC) gives better cross-version deduplication when files are
edited in place, but adds real complexity (rolling hash, boundary tuning,
variable chunk sizes) that isn't needed to prove out encryption, addressing,
and verify-on-read for M0/M1. Fixed-size chunking is simple, deterministic,
and easy to test exhaustively (including corruption detection). The chunk
boundary strategy is isolated behind a single `chunk()` entry point so CDC
can replace it later (targeted for M3+, once versioning/dedup efficiency
matters) without touching callers.

- **Content addressing**: BLAKE3 over each chunk's bytes produces its
  `ChunkId`. BLAKE3 was chosen over SHA-256 for speed (relevant for
  scrubbing large storage roots per D7) while remaining a well-audited,
  widely used hash.
- **Verify-on-read**: re-hashing chunk bytes and comparing against the
  expected `ChunkId` is a single function used both on normal reads and by
  the future scrubbing job (D7), so there is one code path for corruption
  detection.

## Dependency justification

| Dependency | Why |
|---|---|
| `argon2` | Reference Rust implementation of Argon2id, RustCrypto project |
| `chacha20poly1305` | RustCrypto XChaCha20-Poly1305 AEAD implementation |
| `ed25519-dalek` / `x25519-dalek` | Widely used, audited curve25519 implementations |
| `blake3` | Reference Rust implementation by the algorithm's authors |
| `rand_core` / `getrandom` | CSPRNG sourcing, no custom RNG code |
| `zeroize` | Best-effort secret zeroing on drop for key material |
| `thiserror` | Structured error types without boilerplate |

Boring, proven components only (Standard B5); nothing here is
project-invented cryptography.
