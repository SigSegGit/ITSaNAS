# Status

Read this first when resuming work cold.

## M0 — crypto + chunking library: DONE (merged)

Merged via [PR #1](https://github.com/SigSegGit/ITSaNAS/pull/1) into `main`
(`113c47e`). Branch protection is active on `main` (PR required, 1
CODEOWNERS approval, `ci` status check required, force pushes blocked;
administrators can bypass the review requirement only, so the sole owner
isn't locked out by GitHub's no-self-approval rule). CLA Assistant is
installed; it was misconfigured until PR #4 (see below) — genuinely fixed
now, not just showing green.

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
- Mirroring, scrubbing, repair (M3) — will add its own `FaultPoint`
  variants (node dies mid-upload, permission/ownership change, quota
  exceeded, version restore failure) following the pattern this milestone
  established.
- Everything past M1: quotas, daemon, CLI, Android — placeholder crates
  only.

## CLA workflow bugfix: DONE (merged)

Merged via [PR #4](https://github.com/SigSegGit/ITSaNAS/pull/4) into `main`
(`39d8bc5`). `.github/workflows/cla.yml` failed on
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

**Gotcha that applied while PR #4 was open, for next time**: a workflow
fix's own PR can never show a green `cla-check`, no matter how correct the
fix is — `pull_request_target` always executes the workflow file as it
exists on the *base* branch (`main`), not the PR's branch, deliberately, so
a PR can't rewrite its own workflow to exfiltrate secrets. The fix only
takes effect once merged.

Confirmed on PR #5 (the real test, opened after #4 merged): the
`remote-repository` bug is genuinely gone (no more `PERSONAL_ACCESS_TOKEN`
error, and the printed action config no longer shows
`remote-organization-name`/`remote-repository-name` at all). But a second,
different bug surfaced immediately after: `Branch cla-signatures not
found` — the action stores signatures on a dedicated branch and expects it
to already exist; it can't create one from nothing. Fixed by creating an
empty `cla-signatures` branch from `main` directly via the GitHub API
(low-risk, easily reversible — a branch pointing at an existing commit).

**Another gotcha, also for next time**: tried to retrigger the check via
the bot's documented `recheck` PR-comment mechanism
(`issue_comment: types: [created]` + an `if:` checking the comment body is
exactly `recheck`). This session's own comment-posting tool appends a
"Generated by Claude Code" footer and apparently mangles plain text with
stray characters, so the posted body was never the exact literal string
the workflow checks for — the step silently skipped (job still reported
"success" even though the actual CLA check never ran, which is its own
sharp edge: a skipped required step isn't a failed one). Triggering a
fresh `pull_request_target: synchronize` event by pushing a real commit
sidesteps this entirely, since that path's `if:` condition doesn't depend
on comment text at all.

**Third result, after both real bugs above were fixed — likely correct
bot behavior, not a bug**: the action now runs to completion and reports
"Committers of pull request 5 have to sign the CLA." Every commit in this
repo so far is authored as `Claude <noreply@anthropic.com>` (this
session's git identity), which GitHub has no way to associate with the
`SigSegGit` account, so `allowlist: SigSegGit` (which matches by GitHub
identity) doesn't match. This is a genuine policy question, not something
to silently patch around: does the owner want to add this commit identity
to the allowlist, sign once as a one-time exception, or leave this
specific check red? It doesn't block merging either way — `cla-check`
isn't a required status check, only `CI / ci` is.

## M2 — NAT traversal via a self-hosted relay: DONE (on branch, not yet
merged)

On branch `claude/m2-relay-nat-traversal`. Implements D4 (never fall back
to iroh's public relay infra), D5 (self-hosted relay), D12 (invite-only
join), and D13 (CGNAT self-test) — see `ARCHITECTURE.md`'s expanded
`itsanas-net` section for full design.

- **`RelayPolicy`** (`Disabled` | `SelfHosted { url, auth_token }`) is now
  required by `Node::bind`. There is no variant reaching iroh's public
  relay infrastructure — D4 is a property of the type.
- **Real relay integration test**: runs an actual `iroh-relay` server
  in-process (the same server binary deployed to the Freebox VM) and
  connects using an `EndpointAddr` containing *only* a relay URL — no
  direct IP candidates — proving relay-only contact information is
  sufficient for a full exchange. Lives as a library unit test (not
  `tests/*.rs`) because it needs `CaTlsConfig::insecure_skip_verify()` to
  trust the test relay's self-signed cert, an option deliberately never
  exposed through the public API.
- **Found and fixed a real hang**, not just a test artifact: `Node::bind`
  was using iroh's `N0` preset, which bundles a DNS/pkarr discovery service
  depending on n0.computer's own servers — unreachable in this project's
  sandboxed environment (hung for 30+ minutes before being caught and
  killed), and also not something this project should depend on per D5's
  "run our own bootstrap infrastructure" intent regardless of environment.
  Switched to iroh's `Minimal` preset (mandatory crypto provider only, no
  discovery services). Separately, `connectivity_report()` was calling
  `Endpoint::online()` unconditionally, which waits specifically for a
  relay connection to succeed — a real latent bug that would have hung
  forever for any LAN-only (`RelayPolicy::Disabled`) node; fixed to skip
  the wait when no relay is configured.
- **`ConnectivityReport`** (D13): direct-vs-relay-only reachability,
  built from already-known endpoint addresses (doesn't itself probe
  anything).
- **`Invite`** (D12): Ed25519-signed credential (reusing `itsanas-crypto`)
  committing to a bootstrap peer's identity and an expiry.
- **`scripts/relay/`**: real deployment artifacts for the Freebox VM
  (`relay.example.toml` for the `iroh-relay` binary — Let's Encrypt TLS,
  QUIC address discovery, shared-token access control — and a systemd
  unit). Deploying to the actual hardware is an owner action; no Claude
  Code session has network access to the Freebox VM, same boundary as the
  CLA Assistant app install.

24 new/changed tests across `itsanas-net`, all passing, `scripts/ci.sh`
green end-to-end.

## Next steps

1. Get the M2 branch reviewed and merged. Confirm on this PR (opened after
   PR #4 merged) that `cla-check` genuinely passes now, not via the
   lock-only shortcut.
2. M3: mirroring + repair + scrubbing, hardened against hostile storage
   backends (D7) — new logic in `itsanas-repair`, reusing
   `itsanas-storage`'s write-then-verify-readback and
   `itsanas-chunking`'s verify-on-read, plus new fault points for the
   receipt script.
