# ITSaNAS at a glance

A visual tour of what this project is, how it works, and how it's
tested — written for any reader, technical or not. Every diagram below
renders directly on GitHub; no tooling needed.

**One sentence**: ITSaNAS turns a group of friends' spare disk space
into a private, encrypted, self-hosted alternative to Dropbox — your
files sync through an ordinary folder on your computer, get encrypted
locally, and are mirrored to machines you actually trust, with no cloud
provider involved.

---

## What a user sees

You install one app. You get one folder. Anything you put in it is
encrypted and backed up to your friends' machines automatically; their
files land on yours the same way, unreadable to you.

```mermaid
flowchart LR
    subgraph you["🧑 Your computer"]
        folder["📁 Synced folder<br/>(a normal folder)"]
        gui["🖥️ ITSaNAS app<br/>(status, unlock, alerts)"]
        daemon["⚙️ Background service"]
        vault["🔒 Encrypted vault<br/>(your files + friends' shards,<br/>all unreadable at rest)"]
    end
    subgraph friends["🏠 Friends' machines"]
        m1["🔒 Encrypted mirror"]
        m2["🔒 Encrypted mirror"]
    end
    folder <-->|"watches & syncs"| daemon
    gui <-->|"local API"| daemon
    daemon <-->|"encrypts ⇄ decrypts"| vault
    vault <-.->|"P2P, end-to-end encrypted"| m1
    vault <-.->|"P2P, end-to-end encrypted"| m2
```

Key promises, each one enforced by an automated test (see the testing
section below — none of these is just a claim):

| Promise | What it means concretely |
|---|---|
| **Local-first** | Your files live on your machine; the network adds off-site redundancy and multi-device access, it is not "the storage" |
| **End-to-end encrypted** | File *contents and names* are encrypted before anything leaves your machine — a friend hosting your data can't read a single byte or even see what your files are called |
| **Password-bound** | A stolen copy of the entire data directory is useless without your password |
| **Self-healing visibility** | Corrupted or un-syncable files are detected in the background and shown in the app — problems are never silent |

## How a file actually travels

```mermaid
flowchart TD
    drop["You drop 'vacation.mp4'<br/>into the synced folder"]
    watch["Sync engine notices<br/>(filesystem watcher + polling)"]
    chunk["File is split into chunks<br/>(content-defined chunking)"]
    encrypt["Each chunk is encrypted locally<br/>(AES-GCM, key derived from<br/>your password — never leaves<br/>your machine)"]
    store["Encrypted shards written<br/>to the local vault"]
    mirror["Shards replicate to friends'<br/>machines over P2P<br/>(direct, or via a self-hosted<br/>relay when NAT blocks direct)"]
    scrub["Background scrub keeps<br/>re-verifying shard integrity;<br/>damage is reported by file name<br/>in the app"]

    drop --> watch --> chunk --> encrypt --> store --> mirror
    store --> scrub
```

The same pipeline runs in reverse to read a file back, and the sync
engine also mirrors deletions and edits in both directions — the folder
behaves like any normal folder; the cryptography is invisible.

## The parts, for the technically curious

```mermaid
flowchart TB
    subgraph clients["Clients"]
        gui2["itsanas-gui<br/>(Windows/desktop app, egui)"]
        android["Android app<br/>(Kotlin/Compose)"]
    end
    subgraph core["Local daemon (Rust)"]
        http["HTTP API<br/>(loopback only)"]
        sync2["Folder-sync engine"]
        scrub2["Background scrub"]
        vault2["Encrypted vault"]
    end
    subgraph libs["Core libraries (Rust crates)"]
        crypto["itsanas-crypto<br/>(AES-GCM, Argon2)"]
        chunking["itsanas-chunking<br/>(content-defined chunks)"]
        storage["itsanas-storage<br/>(shard store, atomic writes)"]
        net["itsanas-net<br/>(P2P on iroh/QUIC,<br/>relay + NAT traversal)"]
    end
    gui2 --> http
    android --> http
    http --> sync2 --> vault2
    http --> scrub2 --> vault2
    vault2 --> crypto & chunking & storage
    vault2 -.-> net
```

---

## How it's tested — the part worth a recruiter's attention

The testing philosophy in one line: **every layer is tested where it
actually runs, against real processes and real binaries — and every
production bug becomes a permanent test layer that would have caught
it.**

```mermaid
flowchart TD
    l1["1 · Unit & integration tests<br/>every crate, every commit"]
    l2["2 · Fault injection ('receipt mode')<br/>corrupt a shard, cut a transfer, tamper in transit —<br/>assert the exact right error surfaces every time"]
    l3["3 · End-to-end smoke suite<br/>real daemon processes over real HTTP:<br/>account isolation, stolen-vault resistance,<br/>encryption at rest, sync both ways, lock enforcement,<br/>accented/Unicode file names"]
    l4["4 · GUI tests<br/>every screen state, against a real in-process daemon<br/>(not a mock)"]
    l5["5 · Windows binaries under Wine<br/>the same tests, run as REAL Windows executables —<br/>catches Linux-vs-Windows OS differences in CI"]
    l6["6 · Installer end-to-end<br/>the actual installer a user downloads is silently<br/>installed in CI; the installed app must boot and respond"]

    l1 --> l2 --> l3 --> l4 --> l5 --> l6
```

Layers 5 and 6 exist because of a real event, and it's the best story
in the repo: the very first install on a real Windows machine could
not store a single byte — every write failed with "Access denied".
Root cause: a one-line filesystem-semantics difference between Linux
and Windows (Windows refuses to flush a file opened read-only; Linux
allows it), invisible to any amount of Linux-side testing. The fix took
minutes. The lasting response was structural:

- The full test suite now also runs as **real Windows binaries** (via
  Wine) on every commit — the buggy code fails 9 of 12 storage tests
  under that layer; the fixed code passes 12/12, proving the layer
  genuinely catches this class of bug.
- The **actual installer** is installed and booted in CI on every
  commit — not just built.
- [`TESTING.md`](../TESTING.md) keeps a **"Known blind spots"
  register**: every place the shipped software runs where no automated
  test executes it is either covered or explicitly written down, and an
  entry may only be removed by adding the test — never by deleting the
  bullet.

| CI runs on every commit | What it proves |
|---|---|
| `fmt` + `clippy` + build + workspace tests | The code is clean and every crate's logic holds |
| Fault-injection receipt | Failures produce the *right* errors, never silent corruption |
| Smoke e2e (Linux) | The real daemon does what the promises table above says |
| Smoke e2e (Windows binary under Wine) | …and the *Windows* build does too |
| Installer e2e (Wine) | The exact artifact users download installs and runs |

## Where to go next

- [`README.md`](../README.md) — what the project is, installing
- [`ARCHITECTURE.md`](../ARCHITECTURE.md) — design decisions (D1–D13) and why
- [`TESTING.md`](../TESTING.md) — the full testing doctrine, layer by layer, including the blind-spots register
- [`STATUS.md`](../STATUS.md) — milestone-by-milestone history, including honest write-ups of every production bug found and what was built in response
