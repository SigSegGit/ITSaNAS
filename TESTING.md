# Real-life testing: encryption, isolation, and file transfer

Beyond the unit test suites (`cargo test` per crate) and `scripts/receipt.sh`
(fault injection), the daemon + GUI were exercised end-to-end against a real
running process and real files on disk — the two things that actually matter
for "is this actually safe and does it actually work": can one user read
another's files, and does a file survive the round trip unchanged. Two real
bugs were found this way (both fixed — see below) that no unit test caught,
because both only show up when you look at what's actually written to disk
or sent over the wire, not just at each function's return value in isolation.

Reproduce any of this yourself with two `itsanas-daemon` instances pointed
at separate data/sync directories and ports, e.g.:

```sh
ITSANAS_DATA_DIR=/tmp/userA/data ITSANAS_SYNC_DIR=/tmp/userA/synced ITSANAS_PORT=4292 \
  ./target/debug/itsanas-daemon &
```

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
