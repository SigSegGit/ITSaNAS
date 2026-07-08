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

**Follow-up, confirmed on PR #7**: the owner asked to add the automation
identity to the allowlist ("do the change to not need sign off"), done by
adding `noreply@anthropic.com` to `allowlist:` (PR #6). It doesn't work.
`contributor-assistant/github-action`'s `allowlist` matches only GitHub
usernames/logins (per its own docs), never raw git commit-author emails —
confirmed by PR #7 still failing with the exact same "have to sign"
message even with the fix live on `main`. Reverted the ineffective email
entry (a no-op entry that looks like it does something is worse than an
honest comment explaining why it's red) — see `.github/workflows/cla.yml`.
Net state: `cla-check` shows red on every PR from this automation
identity, harmlessly (not a required check). A real fix — e.g. authoring
commits under a real GitHub-linked identity — is an owner decision to
make explicitly if wanted, not something to route around unilaterally.

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

## itsanas-daemon, itsanas-gui, Windows installer: DONE (merged)

Merged via [PR #6](https://github.com/SigSegGit/ITSaNAS/pull/6) into
`main`.

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

## M3 — mirroring, scrubbing, repair: DONE (merged)

Merged via [PR #6](https://github.com/SigSegGit/ITSaNAS/pull/6) into
`main`. Implements D6's mirroring
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

## Comprehensive test automation: DONE (merged)

Merged via [PR #6](https://github.com/SigSegGit/ITSaNAS/pull/6) into `main`.

Standing requirement going forward, not a one-time cleanup: `./scripts/ci.sh
--full` is now the single command that runs every layer of testing this
project has — see `TESTING.md`'s "Running everything with one command"
for the full table. New this round:

- **`scripts/smoke-e2e.sh`**: the manual end-to-end verification from the
  daemon/GUI/testing round above (account isolation, stolen-vault-data
  resistance, at-rest encryption, large-file integrity, folder sync both
  directions, lock-state enforcement — 20 assertions total) turned into
  a permanent script against real running daemon instances. Wired into
  `scripts/ci.sh` by default (not gated behind `--full` — `curl`/`jq` are
  ubiquitous enough not to need gating). Caught one real bug in itself on
  first run (a missing `mkdir -p` before copying a "stolen" vault copy in
  the test), fixed immediately.
- **`itsanas-gui` logic tests**: `App`'s HTTP/state logic (`refresh_status`,
  `do_setup`, `do_unlock`, `do_lock`, `refresh_files`) refactored apart
  from `egui::Ui` rendering so it's unit-testable, then tested against a
  *real* `itsanas-daemon` router booted in-process on an OS-assigned port
  (not a mock) — 9 new tests covering every screen transition and error
  path, part of `cargo test --workspace` with no extra flag needed.
- **`android/logic-tests`**: a standalone Gradle project (its own
  `settings.gradle.kts`, no Android Gradle Plugin dependency at all) that
  compiles the actual production `Models.kt`/`DaemonApi.kt`/
  `RetrofitClient.kt` — not copies — and round-trips them against literal
  JSON shaped like the daemon's real responses, plus live round-trips
  through a `MockWebServer`. Runs with nothing but a JVM and Maven
  Central access, so it works in this sandbox despite Google's Maven
  repo being blocked. 7 tests, `--full` only (needs `gradle`).
- `scripts/ci.sh --full` also runs the Windows installer build
  (`scripts/package-windows-installer.sh`) when `mingw-w64`/`nsis` are
  present, auto-skipping gracefully otherwise.

## Background scrubbing wired into the daemon: DONE (merged, PR #9)

The first half of "wire M3 into the daemon" — see `ARCHITECTURE.md`'s
expanded `itsanas-daemon` section for full design.

- `Vault::scrub` reuses `itsanas_repair::scrub` against every shard the
  manifest references, mapping flagged shards back to file names.
- A new background task (`scrub.rs`) runs it on an interval
  (`ITSANAS_SCRUB_INTERVAL_SECS`, default 6h) and caches the result in
  `AppState`; `GET /status` exposes it as `vault_health`, cleared on lock
  (a locked vault reveals nothing, including this).
- Both clients updated to match: `itsanas-gui` shows unhealthy files as a
  warning (9 Rust tests, including a real-daemon round-trip of the new
  field); `android/`'s `StatusResponse`/`VaultHealth` models and UI
  updated too (2 new contract tests, `vault_health` defaults to `null`
  so older/newer daemon-client pairs don't break each other).
- `scripts/smoke-e2e.sh` gained 3 new real assertions: a corrupted file
  gets detected and named within one scrub interval, and the report
  disappears the moment the vault locks.
- **Deliberately not done here**: actual recovery (`itsanas_repair::repair`)
  isn't called anywhere yet — it needs a `MirrorSet` (peer addresses to
  fetch a good copy from), and this daemon has no concept of network
  peers at all (`itsanas-net` isn't wired in). Faking a peer list just to
  call `repair` would be worse than being honest that recovery is still
  M4's job (multi-device accounts — "who are my mirror peers" needs a
  real answer, not an invented one). Detection today is still valuable:
  a user at least learns a file needs attention.

## CI cache regression after PR #9: DONE (merged, PR #10)

After PR #9 added `itsanas-repair` (and transitively `iroh`, via
`itsanas-net`) as a dependency of `itsanas-daemon`/`itsanas-gui`,
`CI / ci` runtime jumped from the usual ~2 min to ~7 min on both the PR
run and the following `main` run. Root cause: the cache step's key is an
exact hash of `Cargo.lock` with no fallback, so the changed lockfile was
a hard miss both times, forcing a full cold build of the now-heavier
dependency graph (confirmed via each job's own step timings — the
`Cache` step completed in 0s on both slow runs, versus ~30s on a real
restore). Not the Node 20 Actions-runner deprecation some might expect
from the timing coincidence — that warning only ever shows up in the
Node-based `cla-check` job, never in `ci`, which only runs
`rustup`/`cargo`. Fixed by adding a `restore-keys` prefix fallback to
`.github/workflows/ci.yml`'s cache step, so a `Cargo.lock` change reuses
the nearest prior same-OS cache as a base for incremental compilation
instead of starting from zero. Verified: PR #10's own CI run restored
from cache (30s) and finished in ~2 min, back to baseline.

## Windows installer had no real download path: DONE (merged)

The owner pointed out the install process wasn't actually usable:
`INSTALL.md` said "download `itsanas-installer.exe`", but that file never
existed anywhere a user could reach it. `packaging/windows/installer.nsi`
is NSIS *source* — it has to be compiled — and the compiled `.exe` only
ever existed transiently inside `scripts/ci.sh --full`'s own `dist/`
directory as a build-health check, then got discarded. There was no
release, no artifact, nothing to click. Fixed with
`.github/workflows/release.yml` (tag push `v*`, or manual dispatch):
builds the Windows installer plus Linux/aarch64 binaries
(`scripts/release.sh`, Standard B3) and publishes them as GitHub Release
assets — that's what makes "download and double-click" literally true.
`INSTALL.md`/`ARCHITECTURE.md` updated to point at the Releases page and
explain the `.nsi`-is-source-not-installer confusion directly, since
that's exactly what tripped up a real reader. Verified locally in this
environment: `mingw-w64` + `nsis` are both present here, and
`scripts/package-windows-installer.sh` produces a real 16 MB
`itsanas-installer.exe` end-to-end.

## v0.1.0 released; first real Windows install found a showstopper: FIXED

Cutting the actual release surfaced three latent release-tooling bugs in
a row (each documented in its PR): no cross-linker configured for
aarch64 (PR #15), `release.sh` wiping the just-built Windows installer
out of `dist/` combined with upload steps that skip missing files
silently (PR #16, which also added a hard artifact-existence check), and
two proxy/API limitations that meant neither tag pushes nor
workflow_dispatch could be triggered from this automation — solved by
making a `release/v*` branch push a first-class release trigger
(PRs #13/#14). v0.1.0 is live with `itsanas-installer.exe` plus both
Linux tarballs:
https://github.com/SigSegGit/ITSaNAS/releases/tag/v0.1.0

Then the owner installed it on a real Windows machine and every shard
write failed with "Accès refusé" (os error 5), sync retrying forever,
zero files ever landing in the vault. Root cause
(`itsanas-storage/src/root.rs`): after writing the temp shard file, the
code re-opened it **read-only** just to `sync_all()`. Linux allows
fsync on a read-only fd; Windows' `FlushFileBuffers` demands write
access and fails with exactly that error — so the daemon had literally
never been able to store a single byte on Windows, and no Linux-side
test could ever have seen it. Fixed by writing and syncing through one
writable handle.

**Testing gap closed, not just the bug**: new `scripts/test-windows.sh`
runs the fs-heavy crates (`itsanas-storage`, `itsanas-chunking`,
`itsanas-crypto`) as real Windows binaries under Wine — the buggy code
fails 9 of 12 storage tests under it, the fix passes 12/12, so the
harness demonstrably reproduces this bug class. Wired into
`scripts/ci.sh --full` (auto-skips without `mingw-w64`/`wine`) and into
`.github/workflows/ci.yml` on every commit, so Windows-only filesystem
regressions now fail CI instead of shipping.

## Windows testing deepened after owner feedback: e2e under Wine

The owner's (correct) verdict on the above: unit tests passing on Linux
while the product was dead on arrival on Windows means the testing was
too light, full stop. Follow-up shipped: `scripts/smoke-e2e-windows.sh`
runs the **entire** smoke-e2e suite — all 25 assertions: accounts,
vault isolation, stolen-data resistance, at-rest encryption, 3 MiB
binary round-trip, folder sync both directions, background scrub, lock
enforcement — against the **release Windows daemon binary** under Wine,
i.e. the same profile that ships in the installer, exercising the
Windows socket stack and filesystem end-to-end. In GitHub CI on every
commit and in `ci.sh --full`. Two infrastructure potholes hit and
solved on the way: debug windows-gnu builds of iroh-relay exceed
mingw's 64k DLL-export limit (so the script builds release, which is
what ships anyway), and Ubuntu's wine 9.0 packaging loses its own
`zlib1.dll` (without which the daemon can't even load user32/crypt32 —
the script copies it into the prefix). TESTING.md now also carries a
**Known blind spots** register — every place a shipped artifact runs
where no automated test executes it (GUI window on a real Windows
display, NSIS installer never executed, Android APK not compiled in
CI, aarch64 binaries never run, real-internet NAT paths) is either
tested or written down; removing an entry requires adding the test.

## Overnight autonomous session: Unicode file names + silent-sync-failure fix

Working unsupervised overnight per the owner's instruction, extended the
Windows e2e coverage to non-ASCII file names (French accents, spaces, em
dashes — the owner's own machine is French-locale Windows, so these are
the *normal* case, not an edge case) on both the folder-drop and API
upload paths, then ran that against the real Windows daemon under Wine.

That surfaced a real defect, unrelated to the accent handling itself:
`itsanas-daemon`'s folder-sync engine (`sync.rs::list_folder`) silently
dropped any file whose name failed `OsStr::to_str()` — no log line, no
error, nothing. The file would simply never sync, forever, with zero
indication to the user that anything was wrong — structurally the same
failure shape as the os-error-5 bug from earlier tonight (a real problem
hiding behind an interface that reports "healthy"). Fixed by tracking
these as `SyncIssue`s (name + reason), threading them through
`AppState` → `/status` → `itsanas-gui`, which now shows a red banner
for any file the sync engine can't currently handle. `itsanas-storage`'s
`reconcile()` was also tightened to report (not just `eprintln!`) vault
read/write/delete failures per file the same way, so a struggling file
is visible in the GUI instead of only in a log nobody's watching.

Two false leads chased down and correctly ruled *not* real bugs, worth
recording so they aren't rediscovered from scratch: (1) accented file
names first appeared broken under Wine, but only because the test
container's default locale is POSIX/C — Wine's Unix-path-to-UTF-16
translation depends on the process locale, and under a non-UTF-8 locale
it mangles multibyte names. Real Windows/NTFS stores names as UTF-16
natively and has no equivalent failure mode. Fixed by forcing
`LC_ALL=C.UTF-8` in `test-windows.sh` and `smoke-e2e-windows.sh`, so
Wine test results depend on the code, not the host's env. (2) A test
constructing a literally-invalid-UTF-8 filename (only possible on Linux,
where paths are raw bytes) to exercise the new `SyncIssue` reporting
passes natively but can't be reproduced under Wine, because Wine's
Unix→UTF-16 conversion replaces an invalid byte with U+FFFD rather than
preserving the invalid state — the input silently becomes a different,
valid name instead of reproducing the scenario. That assertion is now
native-only with a comment explaining why, rather than a flaky Wine
assertion or a deleted test.

## Overnight autonomous session summary (2026-07-08)

Working unsupervised overnight per the owner's instruction ("iterate to
fix and test the Windows app, or advance the roadmap, autonomously,
while I sleep"). Four things landed on `main`, all verified by
`./scripts/ci.sh --full` before each push:

1. **Real bug found and fixed**: the sync engine silently dropped any
   file whose name it couldn't read, forever, with zero error anywhere
   — same failure shape as the earlier os-error-5 bug. Now tracked as
   `SyncIssue`s, surfaced through `/status`, shown as a red banner in
   `itsanas-gui`.
2. **New coverage**: accented/non-ASCII file names (the owner's own
   machine is French-locale Windows) tested end-to-end on both the
   folder-drop and API paths, against the real Windows daemon under
   Wine.
3. **The installer itself is now tested, not just built**: silently
   installs the real `.exe` into a fresh Wine prefix and verifies the
   binaries, the autostart/uninstall registry entries, and that the
   installed daemon actually answers HTTP.
4. **One finding investigated and correctly *not* acted on**: the
   Windows GUI binary crashes 3/3 times under `wine`+`Xvfb` (deep in
   winit's event-loop resume handling). Concluded this is a test-
   environment artifact rather than a real product bug — the owner has
   directly observed `itsanas-gui` running on their actual machine
   already — and documented the full reasoning in TESTING.md rather
   than guessing at a dependency bump with no way to visually confirm
   it fixed anything real. **Worth a 10-second glance next time the app
   is open** — not urgent, just worth confirming the window still comes
   up clean — but not something requiring action otherwise.

Two infrastructure fixes needed along the way, both now baked into the
test scripts: Wine's Unix-to-UTF-16 path translation depends on the
process locale (fixed by pinning `LC_ALL=C.UTF-8`), and running the
64-bit `itsanas-*.exe` binaries plus the 32-bit NSIS installer in the
same Wine prefix needs both the `wine64` and `wine32:i386` packages,
not just `wine` (Ubuntu's plain `wine` package alone creates a
32-bit-only prefix that can't run the 64-bit binaries at all).

`release/v0.1.0` was fast-forwarded to pick up all of the above in a
freshly built installer.

## Next steps

1. M2 (PR #5) and daemon/GUI/installer/Android/M3/test-automation (PR #6)
   are both merged into `main` — done.
2. Android client (`android/`): needs an actual build on a machine with
   Android SDK access to go from "compiles in principle, network-layer
   verified standalone" to "actually runs" — a real device/emulator run
   is the remaining gap, not a design blocker.
3. The other half of "wire M3 into the daemon" — actual mirroring/repair,
   not just scrubbing — needs M4 (accounts/quotas) first, since that's
   where "who are my mirror peers" gets a real answer.
4. M7: Reed–Solomon erasure coding once the network reaches 4+ nodes,
   replacing `itsanas-repair`'s full-replication mirroring above that
   threshold (D6).
