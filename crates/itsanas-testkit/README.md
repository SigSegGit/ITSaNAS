# itsanas-testkit

Fault-injection registry (`FaultPoint`) shared by other crates' optional
`test-mode` Cargo feature. Not for production use — see `ARCHITECTURE.md`
at the repo root for the full design and the compile-time safety property
that keeps this out of release builds.
