# Architecture

This document maps the project's locked design decisions to the crates that
implement them, and records why each major dependency was chosen. Keep it
current: every new crate or dependency should be justified here.

## Workspace layout

```
crates/
  itsanas-crypto     encryption, key derivation, identities, key wrapping
  itsanas-chunking   content addressing, chunking, verify-on-read
  itsanas-storage    local storage root management, shard I/O
  itsanas-net        iroh transport: LAN store/retrieve (M1); relay/NAT traversal is M2
  itsanas-testkit    fault-injection registry for other crates' test-mode features
  itsanas-receipt    receipt-runner: drives the scenario under test-mode for scripts/receipt.sh
  itsanas-repair     mirroring (M3), scrubbing, and repair; Reed-Solomon erasure coding is M7
  itsanas-quota      per-user contributed/usable space accounting (placeholder)
  itsanas-daemon     local HTTP API: account, encrypted vault, folder sync engine
  itsanas-gui        desktop companion app (account setup/unlock, synced folder status)
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
| D9 | Android thin client over the daemon's authenticated API, min API 29 | `itsanas-daemon` (API), `android/` (Kotlin/Compose client) |
| D10 | Crypto stack: XChaCha20-Poly1305, X25519/Ed25519, Argon2id, BLAKE3 | `itsanas-crypto` (cipher, identities, KDF), `itsanas-chunking` (content addressing) |
| D11 | No metered dependencies; portable scripts + thin CI wrapper | `scripts/ci.sh`, `scripts/release.sh`, `.github/workflows/ci.yml` |
| D12 | Open code, private networks (invite-only) | `itsanas-net` (`Invite`), `README.md` |
| D13 | Connectivity self-test (CGNAT detection) on first run | `itsanas-net` (`ConnectivityReport`) |

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

## `itsanas-storage`

Implements the active half of D7 (hostile/unreliable storage backends):

- **Content-addressed layout**: shards live at
  `<root>/shards/<first-2-hex-chars>/<64-hex-chars>`, sharded by the first
  byte of their `ChunkId` so no single directory accumulates unbounded
  entries.
- **Write-then-verify-readback**: `put` writes to a temp file in the same
  directory, `sync_all`s it, atomically renames it into place, then
  **reads it back and re-verifies the hash** before returning success.
  `fsync`'s return code alone isn't trusted, per D7's note that it can lie
  over a network filesystem like SMB — only an actual successful read-back
  is treated as proof the write landed.
- **Verify-on-read**: `get` re-hashes a shard's bytes against its `ChunkId`
  on every read, so corruption or tampering that happens *after* a
  successful write (the scenario D7 is actually worried about — a hostile
  backend, not just a flaky one) is caught rather than silently returned.

## `itsanas-net`

Implements D4 (iroh transport), D5 (self-hosted relay), D12 (invite-only
join), and D13 (CGNAT self-test).

- **Transport**: one `iroh::Endpoint` per `Node`, bound via iroh's
  `Minimal` preset — deliberately *not* `N0`, which bundles a DNS/pkarr
  address-lookup service publishing to and resolving from n0.computer's
  own servers. This project runs its own bootstrap/rendezvous
  infrastructure (D5) and must not depend on third-party discovery
  infrastructure any more than it depends on iroh's public relays (D4);
  `Minimal` sets only the mandatory crypto provider and nothing else.
  `Node` owns a `StorageRoot` and serves it over a custom ALPN
  (`itsanas/shard/0`).
- **`RelayPolicy`** (`Disabled` | `SelfHosted { url, auth_token }`) is
  passed to `Node::bind` and is the *only* way to configure relaying.
  There is deliberately no variant that reaches iroh's public relay
  infrastructure (`RelayMode::Default`/`Staging`) — D4's "never fall back
  to it" is a property of the type, not a convention callers have to
  remember. `SelfHosted` builds an `iroh::RelayMap` from a single URL
  (optionally with a shared bearer token, matching the relay's
  `access.shared_token` config — see `scripts/relay/`).
- **Wire protocol**: a minimal hand-rolled request/response format over a
  single bidirectional QUIC stream per request (`Get(ChunkId)` /
  `Put(ChunkId, bytes)` → `Found(bytes)` / `NotFound` / `Stored` /
  `Error(String)`). No serde/postcard dependency was added for this — the
  format is small and stable enough that manual encode/decode is less
  code than wiring up a serialization framework, and it keeps the wire
  format an explicit, auditable artifact of this crate rather than an
  implicit function of a derive macro.
- **The network is not a trust boundary**: `get_remote` re-verifies a
  fetched shard's content address before returning it, exactly like
  `itsanas-storage`'s verify-on-read. D7's "storage backends are hostile"
  assumption extends to peers serving shards over the network — a
  compromised or buggy peer is just another way a shard's bytes might not
  match its claimed id.
- **`ConnectivityReport`** (D13): `Node::connectivity_report` waits for
  the endpoint to settle (only when a relay is configured —
  `Endpoint::online()` specifically waits for a relay connection to
  succeed, so calling it with no relay configured would hang forever) and
  reports whether any direct address was discovered. No direct address at
  all is the signature of CGNAT or an equally restrictive NAT/firewall.
- **`Invite`** (D12): an Ed25519-signed credential committing to a
  bootstrap peer's `EndpointId` and an expiry, reusing `itsanas-crypto`'s
  signing primitives (D10) rather than introducing a second signature
  scheme. It deliberately does not carry that peer's current network
  address — addresses are looked up fresh at connect time, not baked into
  a token that would go stale — and it only proves the invite is
  well-formed and unexpired, not that the issuer is a trusted member;
  that membership check is a lookup added in M4 (accounts).

### Testing NAT traversal without a real second network

`itsanas-net`'s relay test (`node::tests::
two_nodes_exchange_a_shard_via_a_self_hosted_relay_only`) runs a real
`iroh-relay` server in-process (via iroh's `test_utils::run_relay_server`,
the same server code deployed to the Freebox VM — not a mock) and connects
to it using an `EndpointAddr` containing *only* a relay URL, no direct IP
candidates. That's the property that actually matters for NAT traversal:
relay-only contact information is sufficient to complete a full exchange.
Both nodes are on localhost, so iroh may additionally upgrade to a direct
path once the relay has done its job — expected and fine either way.

This test lives as a library unit test, not a `tests/*.rs` integration
test, and for one specific, contained reason: the in-process test relay
uses a self-signed certificate, so establishing a connection to it
requires `iroh::tls::CaTlsConfig::insecure_skip_verify()`. That option is
never exposed through the public `Node::bind` API — a real deployment
always has a properly-signed relay certificate (see
`scripts/relay/relay.example.toml`'s Let's Encrypt config) — so the test
constructs its own `Endpoint` directly, inside the crate, where it can
reach `Node`'s private fields without needing a test-only public
constructor that could be misused outside tests.

A prior version of this test used the default `N0` preset and called
`Endpoint::online()` unconditionally; both hung indefinitely in this
project's sandboxed CI-like environment, because `N0` tries to publish to
and resolve from n0.computer's DNS servers (unreachable here, and not
something this project should depend on regardless — see `Minimal` above)
and `online()` waits specifically for a relay connection that, with no
relay configured, will never come. Both are fixed at the source (`Minimal`
preset; `connectivity_report` skips the wait when `RelayPolicy::Disabled`
is in effect) rather than papered over in the test.

### Deploying the real relay

`scripts/relay/` has the actual deployment artifacts for the Freebox VM:
`relay.example.toml` (the `iroh-relay` server config: Let's Encrypt TLS,
QUIC address discovery, shared-token access control) and
`itsanas-relay.service` (a systemd unit). Deploying them to the real
hardware is an owner action — no Claude Code session has network access to
the Freebox VM — the same boundary as the CLA Assistant app install.

## `itsanas-repair`

M3: implements D6's mirroring policy (encrypted full replication to every
other node below the N≥4 threshold — Reed–Solomon erasure coding at 4+
nodes is M7, post-prototype) and the active half of D7 (scrubbing +
repair, on top of `itsanas-net`'s `Node` and `itsanas-storage`'s
already-verifying `get`/`put`).

- **`scrub`**: re-verifies a given set of `ChunkId`s against a
  `StorageRoot`, classifying each `Healthy`/`Corrupt`/`Missing`. This is
  D7's *active* detection — the storage backend is assumed hostile or
  unreliable, so a shard that was fine when written can still be
  tampered with or degrade later; scrubbing is the periodic check that
  catches that instead of waiting for a read that happens to need it.
  Reuses `StorageRoot::get`'s existing verify-on-read rather than
  re-implementing hash checking — a read failure for any reason other
  than "not found" is treated as unhealthy.
- **`MirrorSet` / `mirror_shard`**: a plain list of peer addresses a node
  mirrors its shards to (D6's "who" is a caller decision — membership
  isn't this crate's concern) and a function that pushes one shard to
  every peer in the set, tolerating individual failures (D7 applies to
  mirror peers exactly like any other storage backend: one bad or
  unreachable mirror shouldn't block replicating to the rest).
- **`repair`**: restores a shard into a node's own local storage by
  trying each mirror in the set in turn until one has a valid copy.
  `Node::get_remote` already re-verifies content on receipt, so a lying
  or corrupted mirror's response is rejected before `repair` ever sees
  it — a bad mirror just means falling through to the next candidate,
  the same as an unreachable one. Only once every candidate has been
  tried does it report `RepairError::NoHealthyMirror`.
- **New fault point**: `repair-all-mirrors-unreachable` forces every
  candidate in `repair`'s loop to behave as unreachable, so the real
  "try each one, then report a clean aggregate failure" logic actually
  runs end-to-end — this is genuinely new logic (the other fault points
  test detection at the storage/network layer; this one tests
  `itsanas-repair`'s own exhaustion/error-reporting behavior). Wired into
  the receipt-runner's scenario as an M3 step (mirror a chunk to two
  nodes, then `repair()` it back) alongside the existing M1 pipeline —
  see `scripts/receipt.sh`.

`itsanas-repair` depends on `itsanas-net` directly (for `Node`,
`EndpointAddr`, now re-exported from that crate rather than requiring
callers to add `iroh` as a direct dependency just to name a type they
already receive from `Node::addr()`).

## `itsanas-daemon`

Implements D9 (the authenticated local API both `itsanas-gui` and the
Android client talk to) on top of `itsanas-crypto`, `itsanas-chunking`,
and `itsanas-storage`. Single-user, single-machine trust boundary for
now — multi-device accounts are M4.

- **Account** (`account.rs`): a password derives the vault's master key
  via `itsanas-crypto`'s Argon2id KDF (D10). There's no username or
  server-side account — just a random salt and an encrypted verification
  tag stored in `account.json`, so a wrong password is rejected cleanly
  (`WrongPassword`) rather than silently producing a useless key. Locking
  discards the derived key from memory (`AppState`'s `RwLock<Option<[u8;
  32]>>`); nothing in the vault is reachable until unlocked again.
- **Vault** (`vault.rs`): a named-file view on top of `itsanas-storage`'s
  content-addressed shards. Each file gets its own randomly generated
  key, wrapped by the master key (D10's key-wrapping design) — the
  master key never directly touches bulk data. The file→chunk-list
  mapping (the *manifest*) is itself encrypted at rest with the master
  key, so a locked vault reveals nothing at all on disk, not even file
  names.
- **HTTP API** (`http.rs`): axum router, bound to `127.0.0.1` only (see
  `main.rs`) — a local daemon for this machine's own clients, never a
  public server. `GET /status`, `POST /account/{setup,unlock,lock}`,
  `GET /files`, `PUT`/`GET`/`DELETE /files/{name}` (raw bytes, not
  multipart — both clients are thin enough that this is the simplest
  thing for them to consume). The default 2 MiB per-request body limit
  axum ships with is disabled here: it's a loopback API for a sync
  client, not a public upload endpoint, and silently rejecting anything
  over 2 MiB would defeat the point of a folder that's supposed to
  behave like a normal filesystem.
- **Folder sync engine** (`sync.rs`): the actual "make it feel like
  Google Drive" mechanism. Rather than requiring manual upload/download
  through a GUI, the daemon watches a real local folder (`notify` crate)
  and mirrors it bidirectionally with the vault, so the OS's own file
  manager — drag-and-drop, copy/paste, open, move, delete — just works.
  This is a deliberately lighter-weight design than a virtual
  filesystem/FUSE mount (explicitly a P2/future non-goal): a plain
  mirrored folder plus a watcher and a poll fallback (to catch
  API-driven changes the watcher can't see) covers the same user-facing
  behavior with far less platform-specific complexity.
  - A small state file (`sync_state.enc`) tracks the content hash each
    file had the last time the folder and the vault agreed, which is
    what distinguishes a one-sided change (edited locally, or `PUT`
    through the API) from a genuine two-sided conflict — resolved in
    favor of the folder, since it should behave like a normal editable
    folder rather than silently losing local edits. **This state file is
    encrypted at rest with the master key, the same way the manifest
    is** — an earlier version stored it as plaintext JSON, which leaked
    file names (as map keys) even while the vault was locked; caught by
    live multi-account testing (see `TESTING.md`) and fixed by encrypting
    it identically to the manifest.
  - Locking the vault stops reconciliation entirely (the sync loop's
    `state.master_key().await` fails and it skips that tick) — a locked
    vault materializes nothing new to disk and uploads nothing, matching
    the "locked means locked" expectation the HTTP API already enforces.
- **Background scrubbing** (`scrub.rs`): M3's D7 half wired into the
  daemon. On a fixed interval (`ITSANAS_SCRUB_INTERVAL_SECS`, default 6
  hours), calls `Vault::scrub` — which reuses `itsanas_repair::scrub`
  rather than re-implementing shard classification, then maps flagged
  shards back to the file name(s) that reference them — and caches the
  result (`AppState::vault_health`) for `GET /status` to expose as
  `vault_health`. Locking clears the cached health report the same way
  it clears the master key: a locked vault reveals nothing, including
  whether its files were healthy as of the last scrub.

  Deliberately detection-only for now. `itsanas_repair::repair` needs a
  `MirrorSet` — a list of peer addresses to fetch a good copy from — and
  this daemon has no concept of network peers at all yet (`itsanas-net`
  isn't wired in here; "who are my mirror peers" is a multi-device
  accounts question, M4's job, not this one's). Inventing a throwaway
  peer-configuration mechanism just to call `repair` would be worse than
  being honest that recovery isn't wired in yet: surfacing real, accurate
  health information today (so a user at least knows a file needs
  attention) is useful on its own, and `repair` slots in cleanly once M4
  gives this a real peer list to draw on.
- **Default directories**: `ITSANAS_DATA_DIR` defaults to a proper
  per-user app-data directory (`%APPDATA%\itsanas` / `~/.config/itsanas`
  / `~/Library/Application Support/itsanas`, via the `dirs` crate) and
  `ITSANAS_SYNC_DIR` defaults to `~/ITSaNAS`, rather than paths relative
  to the current working directory. A Start-Menu/Desktop-launched exe
  has no reliable working directory, and installing to `Program Files`
  (not user-writable without elevation) would have made CWD-relative
  paths actively wrong, not just fragile.

## `itsanas-gui`

The desktop companion app: an `eframe`/`egui` window for account
setup/unlock, showing where the synced folder is, and a live file list —
deliberately *not* a manual upload/download tool, since that's the whole
job the sync engine in `itsanas-daemon` already does. On startup it
checks whether a daemon is already listening on `127.0.0.1:4279` and, if
not, spawns `itsanas-daemon` itself (next to its own binary, falling back
to `PATH`), so double-clicking the GUI is a complete entry point rather
than requiring the daemon to be started separately.

**Windows packaging** (`packaging/windows/installer.nsi`,
`scripts/package-windows-installer.sh`): an NSIS installer, built by
cross-compiling both binaries via `x86_64-pc-windows-gnu`
(`mingw-w64`). Installs per-user under
`%LOCALAPPDATA%\Programs\ITSaNAS` — no admin/UAC prompt, the same
approach Dropbox/Slack/VS Code installers use — with Start Menu and
Desktop shortcuts and a login-time autostart entry. The uninstaller
removes the app itself but deliberately leaves the vault
(`%APPDATA%\itsanas`) and the synced folder (`~/ITSaNAS`) untouched,
matching what any real sync client's uninstaller does.

The compiled `.exe` itself is published by `.github/workflows/release.yml`
(tag push or manual dispatch) as a GitHub Release asset — that's the
piece that makes "download and double-click" literally true for an end
user, as opposed to `scripts/ci.sh --full` building the installer only
as a build-health check and discarding the output.

`itsanas-gui` isn't built for `aarch64` in `scripts/release.sh` — a
Raspberry Pi–class NAS box is headless, so there's no desktop to put a
GUI on; `itsanas-daemon` (which runs everywhere) is what actually matters
there.

**Testing**: `App`'s HTTP/state logic (`refresh_status`, `refresh_files`,
`do_setup`, `do_unlock`, `do_lock`) is deliberately factored apart from
the `egui::Ui`-rendering methods, so it's unit-testable on its own —
`main.rs`'s `#[cfg(test)]` module boots a *real* `itsanas-daemon` router
(via `itsanas-daemon` as a dev-dependency, bound to an OS-assigned
loopback port on a background thread) rather than a mock, and drives
every screen transition and action against it: no account, wrong
password, correct password, lock/unlock, and the file list actually
reflecting what was `PUT` into the vault. These run as part of `cargo
test --workspace` / `scripts/ci.sh`, no extra flag needed.

See `INSTALL.md` for the end-user-facing installation and account/key
management instructions.

## Android client (`android/`)

Implements the rest of D9: a genuinely thin Kotlin/Compose client over
`itsanas-daemon`'s HTTP API — min SDK 29, Retrofit + OkHttp +
kotlinx-serialization. A separate Gradle project (not part of the Cargo
workspace), living at `android/` in this same repo rather than a separate
one, since it's one client among several against the same daemon API.

Unlike `itsanas-gui`, it doesn't run a daemon or attempt a folder mirror
of its own — the user configures a base URL once (the daemon only ever
binds to `127.0.0.1`, so reaching it from a phone always goes through
something else: LAN, Tailscale, an SSH tunnel), then it's account
setup/unlock plus a file list with upload/download/delete, uploads/
downloads streamed via a content `Uri` rather than buffered fully into
memory (`network/UriRequestBody.kt`) to match the daemon's own
intentionally unbounded body size.

**Verification status**: this sandbox cannot reach Google's Maven
repository (same policy that blocks `dl.google.com`), so the Android
Gradle Plugin itself can't be resolved here — confirmed directly (`gradle
tasks` fails on `com.android.application:8.5.2` plugin resolution, not
just at a later SDK-download step). The Compose UI and anything touching
`android.*` APIs (`ContentResolver`, activity result contracts) remains
unverified until built on a machine with real SDK access.

**`android/logic-tests`**: the API contract layer (`network/Models.kt`,
`DaemonApi.kt`, `RetrofitClient.kt`) doesn't reference any Android API,
so rather than leaving that unverified too, it gets a permanent,
standalone Gradle project — its own `settings.gradle.kts`, no dependency
on `:app` or the Android Gradle Plugin at all — that compiles those exact
production files (pointed at directly via a custom `sourceSets` block,
not copies, so they can't silently drift) and round-trips them against
literal JSON shaped exactly like `itsanas-daemon`'s real responses. Only
needs a JVM and Maven Central access (not blocked here, unlike Google's
repo), so it runs in this sandbox and in any CI environment via
`scripts/test-android-logic.sh`. This is what caught a real bug during
development — the kotlinx-serialization Retrofit converter's actual
package is `com.jakewharton.retrofit2.converter.kotlinx.serialization`,
not `retrofit2.converter.kotlinx.serialization` — and it's what would
catch the next one, e.g. a field getting renamed on either side of the
API without the other being updated to match. See `android/README.md`.

## Test mode & the receipt script

Standard B4 asks for a simulation mode that can force each of the system's
failure scenarios and prove they're handled, not just that the happy path
works. This is the concrete mechanism for that, built now while it's cheap
and extended as later milestones add real failure scenarios (repair,
quotas, versioning).

**`itsanas-testkit`** is a small leaf crate (no path dependencies, to avoid
a dependency cycle with the crates it instruments) owning a single
registry: [`FaultPoint`], an enum of every named point in the system where
a specific failure can be forced on purpose, plus `should_fail(point)` to
check whether it's currently requested.

**Compile-time safety property**: other crates depend on `itsanas-testkit`
*only* behind their own optional `test-mode` Cargo feature (see
`itsanas-storage`'s and `itsanas-net`'s `Cargo.toml`). A production release
build never enables that feature, so fault-injection code isn't just
inert at runtime — it doesn't exist in the compiled binary at all. There
is no path by which it could ever activate in a real deployment. Each
call site looks like:

```rust
#[cfg(feature = "test-mode")]
if itsanas_testkit::should_fail(itsanas_testkit::FaultPoint::StorageWriteCorruption) {
    // deliberately corrupt/fail here, then let the *real* surrounding
    // error-handling code run exactly as it would for a genuine failure
}
```

The fault always feeds into the same code path that would run for a real
failure (e.g. injecting on-disk corruption right before
`StorageRoot::put`'s existing write-then-verify-readback check, rather than
special-casing the "test" outcome) — the test proves the real detection
logic works, not a separate mock of it.

Activation is a single environment variable, `ITSANAS_FAULT_POINT=<name>`,
read fresh on each check (no caching, so behavior is simple to reason
about and there's nothing to reset between runs).

**`itsanas-receipt`** (`receipt-runner` binary) runs the M1 two-node LAN
scenario (encrypt → chunk → push to peer → fetch back → verify → decrypt)
either cleanly or under one forced fault point, and checks the outcome
against what that fault point is expected to produce (e.g.
`StorageWriteCorruption` must surface as `NetError::Remote`, not a panic,
a hang, or a different error entirely) — "any failure" isn't good enough,
it has to fail the *right* way.

**`scripts/receipt.sh`** discovers the fault point list from the binary
itself (`receipt-runner --list-fault-points`) rather than hardcoding it —
adding a `FaultPoint` variant is enough for the script to pick it up
automatically — runs the scenario once per point (forcing that failure),
then once more with nothing forced (the clean run), and writes a
`receipt.md` summary. It exits non-zero, and `scripts/ci.sh` calls it on
every commit, so a fault point that stops being handled correctly (or a
new one nobody wired up) breaks CI rather than shipping quietly.

**Current fault points** (all wired into real M0/M1 code):

| Fault point | Forces | Proves |
|---|---|---|
| `storage-write-corruption` | A shard's bytes get corrupted on disk during `put`, before write-then-verify-readback runs | D7: write-then-verify-readback actually catches on-write corruption, not just `fsync`'s say-so |
| `storage-get-io-failure` | `get` fails as if the backend refused to read a present shard | Storage failures propagate as a clean error across the network instead of panicking |
| `net-shard-tamper-in-transit` | A shard's bytes get corrupted after being read from storage but before being sent to the peer | D7: the network is not a trust boundary either — the requester's receipt-side verification catches it |
| `net-peer-disconnect-mid-transfer` | The serving peer drops the connection instead of responding | The requester surfaces this as a transport error rather than hanging |

**Extending this**: as M3 (repair/scrubbing), M4 (quotas), and later
milestones add real logic to `itsanas-repair`/`itsanas-quota`/
`itsanas-daemon`, they add their own `FaultPoint` variants (node dies
mid-upload, permission/ownership change detected, quota exceeded, version
restore failure — the exact scenarios named in Standard B4) and their own
`test-mode`-gated call sites, following the same pattern. `receipt.sh`
needs no changes to pick them up.

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
| `iroh` | QUIC-native P2P transport with built-in hole punching and relay support (D4) |
| `tokio` | Async runtime `iroh` and `itsanas-net`'s connection handling require |
| `axum` | `itsanas-daemon`'s local HTTP API — small, well-maintained, integrates directly with `tokio` |
| `notify` | Cross-platform filesystem watcher backing the folder sync engine |
| `dirs` | Correct per-OS app-data/home directory resolution instead of hand-rolled `%APPDATA%`/`$HOME` logic |
| `eframe`/`egui` | Immediate-mode Rust GUI, cross-compiles cleanly to Windows via `mingw-w64` — avoids a second language/toolchain for the desktop client |
| `ureq` | Small synchronous HTTP client for `itsanas-gui` talking to the loopback daemon API — no async runtime needed in the GUI process |

Boring, proven components only (Standard B5); nothing here is
project-invented cryptography.
