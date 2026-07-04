# itsanas-storage

Local, content-addressed shard storage. Treats the storage backend as
hostile/unreliable (D7): write-then-verify-readback on write, hash
re-verification on every read. See `ARCHITECTURE.md` at the repo root.
