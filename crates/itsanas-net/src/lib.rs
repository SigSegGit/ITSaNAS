//! P2P transport, peer discovery, and relay.
//!
//! Built on `iroh` (D4). M1 only implements direct LAN connectivity
//! ([`Node`], relaying disabled); NAT traversal via a self-hosted relay
//! pinned so it never falls back to iroh's public relay infrastructure
//! (D5) is M2's job, and will extend `Node::bind`'s configuration rather
//! than requiring a rewrite. The invite-only join flow (D12) and the
//! first-run CGNAT connectivity self-test (D13) are also not yet
//! implemented — both belong here once M2 introduces real peer discovery.

mod error;
mod node;
mod protocol;

pub use error::NetError;
pub use node::Node;
pub use protocol::ALPN;
