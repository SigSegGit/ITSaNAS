//! Background daemon: ties `itsanas-storage`, `itsanas-net`,
//! `itsanas-repair`, and `itsanas-quota` together into the long-running
//! service each node runs.
//!
//! Exposes the authenticated local API that `itsanas-cli` and the Android
//! thin client (D9) talk to; the daemon itself is reachable remotely via the
//! `itsanas-net` relay/public endpoint.
//!
//! Resource discipline is a hard requirement: idling must be cheap, and
//! background repair/sync activity must respect configurable CPU, RAM, disk
//! I/O, and bandwidth caps.
//!
//! Placeholder crate: no implementation yet. Real work starts once
//! `itsanas-storage` and `itsanas-net` exist (M1+).
