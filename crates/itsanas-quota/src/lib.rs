//! Storage accounting: contributed space, usable quota, and enforcement.
//!
//! v1 policy is fixed (30 GB usable per user while contributing 100 GB) but
//! the model is built to evolve toward proportional allocation without a
//! rewrite. Also tracks the ≤3× contribution-overhead constraint from D6 so
//! `itsanas-repair` can check it before choosing a redundancy scheme for a
//! new node.
//!
//! Placeholder crate: no implementation yet. Real work starts at M4.
