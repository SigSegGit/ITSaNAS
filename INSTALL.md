# Installing and running ITSaNAS

This covers the desktop client: `itsanas-daemon` (the background service
that holds your encrypted vault) and `itsanas-gui` (the small app you
actually open — account setup/unlock, and where your synced folder lives).

## Windows

1. Download `itsanas-installer.exe` (built from `packaging/windows/` — see
   "Building the installer" below if you're building from source).
2. Double-click it. There's no admin/UAC prompt — it installs to your own
   user account, the same way Dropbox or Google Drive's installer does.
3. It adds a **Desktop shortcut** and a **Start Menu** entry named
   `ITSaNAS`, and sets it to start automatically the next time you log in.
4. Double-click the `ITSaNAS` icon. The first time, it'll ask you to
   create a password — that's covered below, under "Your account and your
   key". After that, a folder called **`ITSaNAS`** appears in your user
   folder (`C:\Users\<you>\ITSaNAS`) — anything you drag into it, copy into
   it, or delete from it is kept in sync automatically. Open, edit, copy,
   move, delete — it behaves like a normal folder, because it is one.

To uninstall: Start Menu → `ITSaNAS` → `Uninstall ITSaNAS`, or the normal
Windows "Add or remove programs". This removes the app itself; it does
**not** delete your vault or your synced folder — same as uninstalling
Dropbox doesn't delete your files.

### Building the installer from source

Requires a Linux or WSL build machine with the Rust toolchain, the
`x86_64-pc-windows-gnu` cross-compiler (`mingw-w64` on Debian/Ubuntu), and
`nsis` (provides `makensis`):

```sh
sudo apt install mingw-w64 nsis
rustup target add x86_64-pc-windows-gnu
./scripts/package-windows-installer.sh
```

This produces `dist/itsanas-installer.exe`.

## Linux / macOS

There's no packaged installer for these yet — build and run from source:

```sh
cargo build --release -p itsanas-daemon -p itsanas-gui
./target/release/itsanas-gui
```

The GUI launches the daemon itself if it isn't already running (it looks
for `itsanas-daemon` next to its own binary, then falls back to `PATH`),
so running just the GUI is enough. If you'd rather run the daemon
manually (e.g. on a headless NAS box with no GUI at all), see
"Running the daemon on its own" below.

## Your account and your key

The first time you open the app, it asks you to **create a password**.
There's no username, no email, no server-side account — this password is
run through Argon2id (a slow, memory-hard key derivation function) to
produce the actual encryption key for your vault (design decision D10).

This means:
- **Everything in your vault is encrypted with a key derived from your
  password.** Not just file contents — file *names* too. Someone with
  full access to your data directory on disk sees only opaque, encrypted
  blobs; nothing about what you've stored is readable without the
  password.
- **There is no password reset.** Nobody — not even in principle — can
  recover your vault if you forget your password. There's no server
  holding a copy of your key or a "forgot password" email flow, because
  there's no server involved at all. Write your password down somewhere
  safe if you're at all unsure you'll remember it.
- **Locking** (from the GUI, or via the API) discards the derived key
  from memory; the daemon can no longer read or write your files until
  you unlock again with the password. This is a local, single-user trust
  boundary — good enough for "my own laptop", not a multi-tenant server.

## Running the daemon on its own

Useful for a headless NAS box, or for scripting/testing against the API
directly. Everything is controlled by environment variables; all of them
have sensible defaults so plain `itsanas-daemon` with no configuration at
all is normally fine.

| Variable | Default | Meaning |
|---|---|---|
| `ITSANAS_DATA_DIR` | `%APPDATA%\itsanas` (Windows) / `~/.config/itsanas` (Linux) / `~/Library/Application Support/itsanas` (macOS) | Where the encrypted vault (account, manifest, chunk shards) lives. |
| `ITSANAS_SYNC_DIR` | `~/ITSaNAS` | The visible, synced folder. |
| `ITSANAS_PORT` | `4279` | Port the local HTTP API listens on (bound to `127.0.0.1` only — never exposed to the network). |

The API itself (`/status`, `/account/setup`, `/account/unlock`,
`/account/lock`, `/files`, `/files/{name}`) is documented in
`ARCHITECTURE.md`.
