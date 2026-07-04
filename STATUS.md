# Status

Read this first when resuming work cold.

## M0 — crypto + chunking library: DONE (merged)

Merged via [PR #1](https://github.com/SigSegGit/ITSaNAS/pull/1) into `main`
(`113c47e`). Branch protection is active on `main` (PR required, 1
CODEOWNERS approval, `ci` status check required, force pushes blocked;
administrators can bypass the review requirement only, so the sole owner
isn't locked out by GitHub's no-self-approval rule). CLA Assistant is
installed and passing (confirmed on PR #2).

## M1 — two-node LAN store/retrieve: DONE (merged)

Merged via [PR #2](https://github.com/SigSegGit/ITSaNAS/pull/2) into `main`
(`af3933f`).

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

## Receipt mode (fault-injection test mode): DONE (merged)

Merged via [PR #3](https://github.com/SigSegGit/ITSaNAS/pull/3) into `main`
(`97a9bdb`). Implements
Standard B4's "simulation mode that can force each failure scenario"
concretely — see `ARCHITECTURE.md`'s new "Test mode & the receipt script"
section for the full design. Summary:

- `itsanas-testkit`: `FaultPoint` registry + `should_fail()`. Other crates
  depend on it only behind their own optional `test-mode` feature, so
  fault injection is compiled out of production builds entirely, not just
  disabled at runtime.
- `itsanas-storage` and `itsanas-net` each instrument real call sites with
  `#[cfg(feature = "test-mode")]`-gated fault points: `storage-write
  -corruption`, `storage-get-io-failure`, `net-shard-tamper-in-transit`,
  `net-peer-disconnect-mid-transfer`.
- `itsanas-receipt` (`receipt-runner` binary): runs the M1 two-node
  scenario cleanly or under one forced fault point, and checks the result
  against the specific error each fault point is expected to produce (not
  just "did it fail somehow").
- `scripts/receipt.sh`: discovers the fault point list from the binary
  itself, runs it once per point plus once clean, writes `receipt.md`,
  fails loudly on any mismatch. Wired into `scripts/ci.sh`, so it runs on
  every commit.

All 4 fault points verified handled correctly, plus the clean run —
`scripts/ci.sh` (which now includes `receipt.sh`) passes end-to-end.

### Not done yet (explicitly deferred)
- NAT traversal / self-hosted relay (M2, D4/D5). `itsanas-net`'s `Node` is
  structured so adding this is a new `relay_mode`/bootstrap option, not a
  rewrite.
- Mirroring, scrubbing, repair (M3) — will add its own `FaultPoint`
  variants (node dies mid-upload, permission/ownership change, quota
  exceeded, version restore failure) following the pattern this milestone
  established.
- Everything past M1: quotas, daemon, CLI, Android — placeholder crates
  only.

## CLA workflow bugfix: DONE (on branch, not yet merged)

On branch `claude/fix-cla-workflow`. `.github/workflows/cla.yml` failed on
its first run for every PR so far (#2 and #3), then mysteriously
"succeeded" on an automatic retry — the retry only looked green because it
took a different, no-op code path (locking an allowlisted author's PR
without touching the signature file), not because anything was actually
fixed. Root cause found in the failed run's logs: the workflow set
`remote-organization-name`/`remote-repository-name` to point at this same
repo, which puts `contributor-assistant/github-action` into "remote
repository" mode — a mode that always requires a `PERSONAL_ACCESS_TOKEN`,
which was never configured (no `CLA_PAT` secret exists). Since signatures
were only ever meant to live in this same repo's `cla-signatures` branch,
not a separate repo, the fix removes that unnecessary remote-repo config
entirely; the default `GITHUB_TOKEN` (already granted `contents: write` in
this workflow) is sufficient for same-repo mode. No new secret needed —
this was a misconfiguration, not a missing credential.

## Next steps

1. Get the CLA workflow bugfix branch reviewed and merged.
2. M2: NAT traversal via a self-hosted relay on the Freebox VM (D5), pinned
   so it never falls back to iroh's public relay infrastructure (D4).
   `itsanas-net` will also grow the invite-only join flow (D12) and the
   first-run CGNAT connectivity self-test (D13) here.
3. M3: mirroring + repair + scrubbing, hardened against hostile storage
   backends (D7) — new logic in `itsanas-repair`, reusing
   `itsanas-storage`'s write-then-verify-readback and
   `itsanas-chunking`'s verify-on-read, plus new fault points for the
   receipt script.
