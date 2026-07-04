//! P2P transport, peer discovery, and relay.
//!
//! Built on `iroh` (D4): QUIC-native, with built-in hole punching and relay
//! support. This crate will pin its relay configuration to a self-hosted
//! relay (initially the Freebox VM, D5) and must never silently fall back to
//! iroh's public relay infrastructure.
//!
//! The Freebox VM is architected as "just another node with a public
//! address," not a special server: removing it must be a config change
//! (switch to DHT discovery / peer relays), not a rewrite, once the network
//! approaches its 10th user (D5). This crate also owns the invite-only join
//! flow (D12) and the first-run connectivity self-test that detects CGNAT
//! (D13).
//!
//! Placeholder crate: no implementation yet. Real work starts at M2.
