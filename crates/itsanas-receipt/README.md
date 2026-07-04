# itsanas-receipt

`receipt-runner`: runs the M1 two-node LAN scenario once, cleanly or under
one forced `itsanas-testkit::FaultPoint`. Driven by `scripts/receipt.sh`,
which runs it once per fault point plus a clean run. Not for production
use — see `ARCHITECTURE.md` at the repo root.
