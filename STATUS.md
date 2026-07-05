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

## M2 — NAT traversal via a self-hosted relay: DONE (merged)

Merged via [PR #5](https://github.com/SigSegGit/ITSaNAS/pull/5) into
`main` (`f510649`). Implements D4 (never fall back
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

## itsanas-daemon, itsanas-gui, Windows installer: DONE (on branch
`claude/daemon-and-clients`, not yet merged)

Deliberate reprioritization at the owner's direction: rather than
continuing straight to M3 (mirroring/repair) after M2, this branch builds
the local daemon + desktop client stack M3's own design depends on anyway
(D9), since tangible, runnable Windows/Android clients were asked for
explicitly. M3 is deferred, not abandoned — see "Next steps" below.

- **`itsanas-daemon`**: single-user password-derived account (D10, Argon2id
  + a verification tag so a wrong password fails cleanly), an encrypted
  per-file vault on top of `itsanas-storage`/`itsanas-chunking` (manifest
  encrypted at rest — file names included), and a local-only (`127.0.0.1`)
  HTTP API: setup/unlock/lock, list/get/put/delete files.
- **Folder sync engine** (`sync.rs`): watches a real local folder
  (`notify` crate) and mirrors it bidirectionally with the vault, plus a
  poll fallback for API-driven changes the watcher can't see — this is
  what makes drag-and-drop/copy/paste/open through the OS work
  transparently, the explicit bar the owner set ("like Google Drive").
  Deliberately a mirrored folder, not a FUSE/virtual-filesystem mount —
  matches the project's own non-goals list.
- **`itsanas-gui`**: `eframe`/`egui` desktop companion app — account
  setup/unlock, shows the synced folder location, live file list. Launches
  the daemon itself if it isn't already running.
- **Windows installer** (`packaging/windows/installer.nsi` +
  `scripts/package-windows-installer.sh`): single-click, no admin/UAC
  prompt (per-user install under `%LOCALAPPDATA%`), Start Menu + Desktop
  shortcuts, launch-at-login. Built and verified to produce a valid PE
  installer in this environment (can't be run interactively here — no
  Windows machine in this sandbox — but both bundled binaries
  cross-compile and the installer builds cleanly via `makensis`).
- **Android client** (`android/`): a genuinely thin Kotlin/Compose client
  over `itsanas-daemon`'s HTTP API (D9, min SDK 29) — Retrofit + OkHttp +
  kotlinx-serialization, account setup/unlock, file list with
  upload/download/delete (streamed via a content `Uri`, not buffered, to
  match the daemon's unbounded body size). Unlike `itsanas-gui` it
  doesn't run a daemon or attempt a folder mirror — the user configures a
  base URL once, since the daemon only binds to `127.0.0.1` and reaching
  it from a phone always goes through something else (LAN/Tailscale/SSH
  tunnel). Full Gradle build is genuinely blocked in this sandbox
  (confirmed directly: `gradle tasks` fails resolving the Android Gradle
  Plugin itself against Google's Maven repo, the same block that affects
  `dl.google.com`) — but the API contract layer (`Models.kt`,
  `DaemonApi.kt`, `RetrofitClient.kt`) doesn't touch any Android API, so
  it was compiled standalone against real dependencies from Maven Central
  (not blocked), catching and fixing one real bug (wrong package for the
  kotlinx-serialization Retrofit converter). The Compose UI remains
  unverified until built on a machine with real SDK access — see
  `android/README.md`.
- **Real-life testing** (`TESTING.md`): live multi-account isolation,
  stolen-vault-data, at-rest-encryption, and large-binary-file-integrity
  testing against running daemon instances (not just unit tests). Found
  and fixed two real bugs this way:
  - the sync engine's state sidecar was plaintext JSON recording file
    names, leaking exactly what the encrypted manifest is designed to
    hide — now encrypted with the master key like the manifest is.
  - axum's default 2 MiB body limit silently rejected any upload bigger
    than that — disabled for this loopback-only API.
- Also switched default data/sync directories from CWD-relative paths to
  proper per-user locations (`dirs` crate) — a real installed app has no
  reliable working directory, and `Program Files` isn't user-writable.

## M3 — mirroring, scrubbing, repair: DONE (on branch, not yet merged)

Still on branch `claude/daemon-and-clients`. Implements D6's mirroring
policy (full replication below the N≥4 erasure-coding threshold, which is
M7) and the active half of D7 (scrubbing + repair) — see
`ARCHITECTURE.md`'s new `itsanas-repair` section for full design.

- **`scrub`**: re-verifies a set of shard ids against a `StorageRoot`,
  classifying each healthy/corrupt/missing — reuses `StorageRoot::get`'s
  existing verify-on-read rather than re-implementing hash checking.
- **`MirrorSet`/`mirror_shard`**: pushes a shard to every peer in a
  caller-provided mirror set, tolerating individual peer failures (D7
  applies to mirrors too).
- **`repair`**: restores a shard from the first mirror in the set with a
  valid copy, falling through corrupt or unreachable candidates —
  `Node::get_remote`'s existing re-verification means a lying mirror's
  response is already rejected before `repair` sees it.
- **New fault point**: `repair-all-mirrors-unreachable`, wired into
  `repair`'s own peer loop (not the storage/network layer, which the
  existing 4 fault points already cover) and into the receipt-runner's
  scenario as a new M3 step. All 5 fault points plus the clean run pass
  via `scripts/receipt.sh`.
- **Found and fixed a real gap while wiring this up**: `itsanas-net`
  didn't re-export `EndpointAddr` even though it's already part of its
  public API (`Node::addr()` returns it, `get_remote`/`put_remote` take
  it) — any downstream crate naming the type had to add `iroh` as its
  own direct dependency just to spell it. Fixed by re-exporting it from
  `itsanas-net` directly.
- 5 new unit tests in `itsanas-repair` (mirroring to multiple peers,
  partial-failure reporting, scrub classification, repair falling
  through a corrupt mirror to a healthy one, repair failing cleanly when
  no mirror has the shard), all passing; full `scripts/ci.sh` green.

## Next steps

1. M2 (PR #5) is merged into `main` — done.
2. Get `claude/daemon-and-clients` reviewed and merged (daemon, GUI,
   Windows installer, Android client scaffold, M3 mirroring/repair,
   docs/testing).
3. Android client (`android/`): needs an actual build on a machine with
   Android SDK access to go from "compiles in principle, network-layer
   verified standalone" to "actually runs" — a real device/emulator run
   is the remaining gap, not a design blocker.
4. `itsanas-daemon` doesn't call into `itsanas-repair` yet — the vault
   currently has no mirror-peer configuration or scrub schedule of its
   own. Wiring M3's mirroring/repair into the actual daemon (not just the
   library + receipt scenario) is the next real integration step, likely
   alongside M4 (accounts/quotas), since "who are my mirror peers" is a
   multi-device/multi-account question.
5. M7: Reed–Solomon erasure coding once the network reaches 4+ nodes,
   replacing `itsanas-repair`'s full-replication mirroring above that
   threshold (D6).
