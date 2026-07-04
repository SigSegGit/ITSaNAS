# Status

Read this first when resuming work cold.

## M0 — crypto + chunking library: DONE (merged)

Merged via [PR #1](https://github.com/SigSegGit/ITSaNAS/pull/1) into `main`
(`113c47e`). Branch protection is active on `main` (PR required, 1
CODEOWNERS approval, `ci` status check required, force pushes blocked;
administrators can bypass the review requirement only, so the sole owner
isn't locked out by GitHub's no-self-approval rule). The CLA Assistant
GitHub App install is still outstanding (owner action, one-click install,
not automatable) — `.github/workflows/cla.yml` will start working once
it's installed and a `CLA_PAT` secret is added.

## M1 — two-node LAN store/retrieve: DONE (pending review/merge)

On branch `claude/m1-lan-store-retrieve`.

- `itsanas-storage`: content-addressed local shard store, write-then-verify
  -readback on write, verify-on-read on read (D7). 12 tests, including
  detecting a shard corrupted on disk after being written.
- `itsanas-net`: `Node` wraps an `iroh::Endpoint` (relay disabled — LAN only
  for M1) serving a small hand-rolled request/response protocol
  (`Get`/`Put` a `ChunkId`) over a custom ALPN. 10 unit tests on the wire
  protocol, plus a 2-node integration test
  (`tests/lan_store_retrieve.rs`) that runs the *entire* pipeline
  end-to-end: encrypt a file with `itsanas-crypto` → chunk the ciphertext
  with `itsanas-chunking` → push shards to a peer over iroh → fetch them
  back from the peer (simulating local-copy loss) → verify → decrypt →
  compare to the original plaintext. Also covers the not-found case.
- Researched the current `iroh` 1.0.1 API directly from its bundled
  examples and internal test suite (`endpoint_two_direct_only`) before
  writing any code, rather than guessing at an API that changes across
  major versions.

Full `scripts/ci.sh` passes: fmt clean, clippy clean (`-D warnings`), 57
tests passing across the whole workspace.

### Not done yet (explicitly deferred)
- NAT traversal / self-hosted relay (M2, D4/D5). `itsanas-net`'s `Node` is
  structured so adding this is a new `relay_mode`/bootstrap option, not a
  rewrite.
- Mirroring, scrubbing, repair (M3).
- Everything past M1: quotas, daemon, CLI, Android — placeholder crates
  only.

## Next steps

1. Get the M1 branch reviewed and merged via PR (same admin-bypass-merge
   pattern as M0, since self-approval isn't possible).
2. M2: NAT traversal via a self-hosted relay on the Freebox VM (D5), pinned
   so it never falls back to iroh's public relay infrastructure (D4).
   `itsanas-net` will also grow the invite-only join flow (D12) and the
   first-run CGNAT connectivity self-test (D13) here.
3. M3: mirroring + repair + scrubbing, hardened against hostile storage
   backends (D7) — new logic in `itsanas-repair`, reusing
   `itsanas-storage`'s write-then-verify-readback and
   `itsanas-chunking`'s verify-on-read.
