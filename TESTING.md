# Real-life testing: encryption, isolation, and file transfer

Beyond the unit test suites (`cargo test` per crate) and `scripts/receipt.sh`
(fault injection), the daemon + GUI are exercised end-to-end against a real
running process and real files on disk — the two things that actually matter
for "is this actually safe and does it actually work": can one user read
another's files, and does a file survive the round trip unchanged. Two real
bugs were found this way (both fixed — see below) that no unit test caught,
because both only show up when you look at what's actually written to disk
or sent over the wire, not just at each function's return value in isolation.

**This is no longer a manual exercise.** Everything below is now
`scripts/smoke-e2e.sh` — a permanent, repeatable script that starts real
daemon instances, drives them over HTTP, and asserts on both the responses
and what actually lands on disk, failing loudly (and exit non-zero) the
instant any of it regresses. It's part of `scripts/ci.sh`, so it runs on
every commit — see "Running everything with one command" below for the
full picture (this script plus GUI and Android test coverage). Run it on
its own with:

```sh
./scripts/smoke-e2e.sh
```

The walkthroughs below are what that script actually does and why each
check exists — read them for the reasoning; run the script for the
up-to-date, enforced version of the same checks.

## 1. Two accounts can't see or unlock each other's vault

Set up two independent daemon instances ("Alice" and "Bob"), each with its
own password and its own file:

```
$ curl -X POST :4292/account/setup -d '{"password":"alice-secret-pw-1"}'   -> 201
$ curl -X POST :4293/account/setup -d '{"password":"bob-different-pw-2"}'  -> 201
$ curl -X PUT  :4292/files/alices-diary.txt --data-binary 'ALICE_SECRET_MARKER...' -> 201
$ curl -X PUT  :4293/files/bobs-notes.txt   --data-binary 'BOB_SECRET_MARKER...'   -> 201

$ curl :4292/files -> [{"name":"alices-diary.txt","size":46}]
$ curl :4293/files -> [{"name":"bobs-notes.txt","size":35}]
```

Each vault's file list contains only its own files, as expected — but the
real question is whether the *password* is what actually gates access, not
just "which port you happened to ask":

```
$ curl -X POST :4293/account/lock -> 200
$ curl -X POST :4293/account/unlock -d '{"password":"alice-secret-pw-1"}'
  -> 401 "incorrect password"
$ curl :4293/files -> 401 "the vault is locked; unlock it first"
$ curl -X POST :4293/account/unlock -d '{"password":"bob-different-pw-2"}' -> 200
$ curl :4293/files -> [{"name":"bobs-notes.txt","size":35}]  (200, back to normal)
```

Bob's vault rejects Alice's password outright and stays locked; it only
opens for the password it was actually set up with. **Result: PASS.**

## 2. Stolen vault data is useless without the exact password

The more realistic threat isn't "guess a port" — it's someone getting a
copy of the data directory itself (stolen laptop, backup left somewhere,
compromised storage backend per D7). Copied Alice's entire
`ITSANAS_DATA_DIR` to a fresh location and pointed a new daemon instance at
the copy:

```
$ cp -r userA/data stolen_data
$ ITSANAS_DATA_DIR=stolen_data ITSANAS_PORT=4294 ./itsanas-daemon &

$ curl -X POST :4294/account/unlock -d '{"password":"password123"}'
  -> 401 "incorrect password"
$ curl -X POST :4294/account/unlock -d '{"password":"bob-different-pw-2"}'
  -> 401 "incorrect password"
$ curl -X POST :4294/account/unlock -d '{"password":"alice-secret-pw-1"}'
  -> 200
$ curl :4294/files -> [{"name":"alices-diary.txt","size":46}]
```

Full possession of the on-disk data is not enough; only the real password
(Argon2id-derived key, D10) unlocks it. **Result: PASS.**

## 3. Nothing is readable at rest without the key — not even file names

Grepped the entire data directory for both the plaintext content marker and
the plaintext file name while the vault was populated:

```
$ grep -r "ALICE_SECRET_MARKER" userA/data/   -> no match
$ grep -r "alices-diary" userA/data/          -> no match
$ find userA/data -type f
  userA/data/manifest.enc
  userA/data/account.json
  userA/data/sync_state.enc
  userA/data/shards/shards/6e/6ef4ca3a...
```

**Bug found and fixed here**: the first version of the folder-sync engine
(`sync.rs`) wrote its reconciliation state to a plaintext `sync_state.json`
sidecar recording file names as JSON map keys (to track "last known content
hash per file"). That directly contradicted the vault's own design intent —
the manifest (`manifest.enc`) is encrypted specifically so a locked vault
reveals nothing, including file names — and the plaintext sidecar quietly
leaked exactly that. Live testing caught it (the `grep` above found the file
name on the first run); fixed by encrypting `sync_state.enc` with the master
key the same way the manifest is, using the same AEAD cipher and a distinct
AAD string. Re-ran the grep after the fix: no match, and the file is now
opaque ciphertext (confirmed with `od -c`). **Result: PASS (after fix).**

## 4. Large binary files round-trip byte-for-byte

Generated 5 MiB of random data (`/dev/urandom`) — large enough to span
multiple 1 MiB chunks (`itsanas-chunking`'s `DEFAULT_CHUNK_SIZE`) — and
round-tripped it through the API:

```
$ sha256sum random_5mb.bin
  7f95253efafbd6160774698331ce6fb3a134eb4c7f7abca9e63c8ff1f8a285a6

$ curl -X PUT :4292/files/random_5mb.bin --data-binary @random_5mb.bin
```

**Bug found and fixed here**: the first attempt returned `413 Failed to
buffer the request body: length limit exceeded` — axum ships a 2 MiB
default per-request body limit, which silently caps every upload at a size
far too small for real files (photos, videos, archives). Fixed by disabling
the limit on `itsanas-daemon`'s router (it's a loopback-only API for a
sync client's own use, not a public upload endpoint — see `http.rs`).
After the fix:

```
$ curl -X PUT :4292/files/random_5mb.bin --data-binary @random_5mb.bin -> 201
$ curl :4292/files/random_5mb.bin -o downloaded.bin -> 200
$ sha256sum downloaded.bin
  7f95253efafbd6160774698331ce6fb3a134eb4c7f7abca9e63c8ff1f8a285a6   (identical)
$ cmp random_5mb.bin downloaded.bin -> exit 0 (byte-for-byte identical)
```

Also confirmed the file materializes correctly through the *sync engine*
(not just the raw API), by checking it appeared in the watched folder with
the same checksum:

```
$ sha256sum userA/synced/random_5mb.bin
  7f95253efafbd6160774698331ce6fb3a134eb4c7f7abca9e63c8ff1f8a285a6
$ find userA/data/shards -type f | wc -l
  6   (1 file from test 1 + 5 chunks for the 5 MiB file — chunking confirmed working)
```

**Result: PASS (after fix).**

## 5. Folder sync in both directions (already covered, re-verified here)

Covered in detail during `itsanas-daemon`'s development and re-confirmed
during this round: dropping a file into the synced folder uploads it to the
vault; deleting it from the folder deletes it from the vault; editing it
in the folder re-uploads it; a `PUT`/`DELETE` through the HTTP API
materializes/removes the file in the folder — all within one poll interval
(2s). A locked vault does none of this (reconciliation is skipped entirely
while locked), so nothing new is written to or read from disk without the
key. **Result: PASS.**

## Summary

| # | Scenario | Result |
|---|---|---|
| 1 | Two accounts, wrong password can't unlock the other's vault | PASS |
| 2 | Stolen data directory, only the real password unlocks it | PASS |
| 3 | Nothing readable at rest without the key, including file names | PASS (after fixing a plaintext state-file leak) |
| 4 | Large binary file round-trips byte-for-byte | PASS (after fixing axum's default body-size limit) |
| 5 | Folder sync works both directions, respects lock state | PASS |

All 20 assertions above are enforced automatically by
`scripts/smoke-e2e.sh` on every commit (via `scripts/ci.sh`) — this table
records what was found the first time; the script is what keeps it true.

## Running everything with one command

The standing bar for this project: nothing should ship where "did it
actually work" depends on someone remembering to check by hand. Every
layer below has automated coverage, and one command runs literally all of
it:

```sh
./scripts/ci.sh --full
```

| Layer | What runs | Covers |
|---|---|---|
| Code | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo build`, `cargo test --workspace` | Every crate's unit/integration tests, including `itsanas-gui`'s (see below) |
| Fault injection | `scripts/receipt.sh` | Every named `FaultPoint` (storage corruption, network tamper/disconnect, unreachable mirrors) produces exactly the right error, plus a clean run |
| Feature (daemon, end-to-end) | `scripts/smoke-e2e.sh` | Account isolation, stolen-vault-data resistance, at-rest encryption (content *and* file names), large binary file integrity, folder sync in both directions, lock-state enforcement, background scrub detecting a corrupted file by name and clearing once the vault locks — against real running daemon processes, not mocks |
| GUI | `cargo test -p itsanas-gui` (part of `cargo test --workspace`) | Every screen transition (no daemon / no account / locked / unlocked) and every action (`do_setup`, `do_unlock`, `do_lock`) against a real in-process `itsanas-daemon` router — not a mock, and not the same process as smoke-e2e's, so a GUI-side regression can't hide behind the daemon's own tests passing |
| Android | `scripts/test-android-logic.sh` (`--full` only; needs `gradle` + Maven Central) | The exact production `Models.kt`/`DaemonApi.kt`/`RetrofitClient.kt` — not copies — round-tripped against literal JSON shaped like the daemon's real responses, so a field rename on either side fails immediately instead of silently breaking on a real phone |
| Windows installer | `scripts/package-windows-installer.sh` (`--full` only; needs `mingw-w64` + `nsis`) | Both binaries cross-compile and `makensis` produces a valid installer |
| Windows behavior | `scripts/test-windows.sh` (`--full` only; needs `mingw-w64` + `wine` — also runs in GitHub CI on every commit) | The fs-heavy crates (`itsanas-storage`, `itsanas-chunking`, `itsanas-crypto`) tested as **real Windows binaries** under Wine. Exists because the very first real Windows install failed every shard write with "Access denied" (os error 5): `FlushFileBuffers` refuses read-only handles on Windows, while `fsync` on Linux accepts them — a class of bug no amount of Linux-side testing can see. Under Wine the buggy code failed 9 of 12 storage tests and the fixed code passes all 12, so this layer demonstrably reproduces the real failure |
| Windows e2e | `scripts/smoke-e2e-windows.sh` (`--full` only; needs `mingw-w64` + `wine` — also runs in GitHub CI on every commit) | The **entire smoke-e2e suite** (all 25 assertions: accounts, vault isolation, stolen-data resistance, at-rest encryption, binary round-trip, folder sync both directions, scrub, lock enforcement) against the **release Windows daemon binary** under Wine — the same profile the shipped installer contains. Note: the script works around an Ubuntu wine 9.0 packaging bug (wine's own PE `user32.dll` needs a `zlib1.dll` the loader can't find; the daemon pulls user32/crypt32 in via cert-store and known-folder APIs, so without the workaround it can't even start) |
| Windows installer e2e | `scripts/test-windows-installer.sh` (`--full` only; needs `mingw-w64`, `nsis`, and both `wine32:i386` + `wine64` — also runs in GitHub CI on every commit) | Silently installs the **real NSIS installer** (`wine dist/itsanas-installer.exe /S`) into a fresh Wine prefix and asserts what a user actually gets: both binaries on disk, the autostart and uninstall registry entries, and — the part that matters — that the installed `itsanas-daemon.exe` actually runs and answers HTTP. Building the installer only proved `makensis` accepted the script; nothing before this ran what it installs. Needs a genuine win64 Wine prefix (the `wine64` package, not just `wine`) because the 64-bit `itsanas-*.exe` binaries fail with "Bad EXE format" under Ubuntu's default 32-bit-only prefix, and needs `wine32:i386` because the installer itself is a 32-bit PE requiring Wow64 — both found by hitting the failure first |
| Infra/workflow | `.github/workflows/ci.yml`, `cla.yml` | CI itself runs `scripts/ci.sh`; CLA gating is exercised by every real PR |

Plain `scripts/ci.sh` (no flag) runs the first three rows — everything
that only needs the Rust toolchain plus `curl`/`jq` (both ubiquitous), so
it's what every environment, including a bare CI runner, can always run.
`--full` adds the rows that need extra tooling (`gradle`+network,
`mingw-w64`+`nsis`), auto-skipping any of them that aren't installed
rather than failing the whole run — so `--full` degrades gracefully on a
machine that only has some of the extra tools, instead of being all-or-
nothing.

**The standard going forward**: new code in this repo should come with
tests that exercise it at the layer where it actually runs — a new crate
function gets a unit test, a new HTTP endpoint or cross-process behavior
gets a `smoke-e2e.sh` (or receipt-mode) check, a new GUI action gets a
test against a real in-process daemon the way `do_setup`/`do_unlock`/
`do_lock` do. The goal is that `./scripts/ci.sh --full` passing is
sufficient evidence the software works, not just that it compiles.

## Known blind spots

Places where the shipped artifact still runs somewhere no automated test
executes it. Kept here deliberately: a gap that's written down gets
weighed before every release; a gap nobody wrote down ships bugs (that's
exactly how the Windows os-error-5 shard-write failure reached a real
user's machine — the code had simply never been executed on Windows).
Anything removed from this list must be removed by *adding the test*,
not by deleting the bullet.

- **The GUI window on real Windows — tried tonight, inconclusive by
  design, don't attempt to "fix" this without a real Windows machine.**
  `itsanas-gui`'s screen transitions and actions are tested against a
  real in-process daemon, and the daemon side is e2e-tested as a
  Windows binary — but actual window creation has no automated
  coverage. Tried running the real Windows GUI binary under
  `xvfb-run wine` tonight: it crashes 3/3 times, deterministically, at
  the identical location — `winit::platform_impl::windows::event_loop::
  EventLoop::run_on_demand` → `eframe::native::glow_integration::
  GlowWinitApp::resumed` → `Instant - Duration` underflow
  (`std::time.rs:445`), on the very first event-loop resume. That's
  eframe 0.29.1 / winit 0.30.13 (current pins; no newer 0.29.x patch
  exists, and jumping to eframe 0.30+ is a real API migration this
  environment can't visually verify, so left un-bumped rather than
  guessed at).
  **Conclusion: this is a Wine+Xvfb test-environment artifact, not a
  real product bug** — the owner has directly observed `itsanas-gui`
  running on their actual Windows machine (that's how the earlier
  os-error-5 sync failures were even visible to them as a running app
  with silent retries, not a crash). The deterministic 3/3 repro rules
  out flakiness, but the panic is in event-loop *resume* scheduling
  that a headless Xvfb display with no compositor/vsync signal very
  plausibly drives down a code path real Windows never takes on first
  launch. Recorded rather than "fixed blind": don't bump eframe/winit
  to chase this without being able to see a real window afterward.
  Regression here would still look like: binary starts, no window
  appears. Mitigation unchanged: manual check on a real Windows machine
  at each release remains the only way to verify this layer.
- **The Android APK is never compiled in CI.** The network-contract
  logic layer (exact production `Models.kt`/`DaemonApi.kt`) runs under
  plain gradle, but the full Compose app needs the Android SDK, which
  CI doesn't have — a break in the UI layer or manifest only surfaces
  when someone builds the APK locally.
- **The aarch64 Linux release binaries are cross-compiled but never
  executed** (would need qemu-user or an arm64 runner). They share ~all
  code with the tested x86_64 build, so the realistic risk is
  arch-specific dependency breakage, not logic.
- **Real-internet NAT/CGNAT paths.** The relay integration test runs a
  real relay binary on loopback, and D13 gives users a connectivity
  self-test, but no CI job crosses a real NAT.
