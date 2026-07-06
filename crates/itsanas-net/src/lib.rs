//! P2P transport, peer discovery, and relay.
//!
//! Built on `iroh` (D4). [`Node`] handles both direct LAN connectivity and
//! relayed connectivity via a self-hosted relay (D5) through a single
//! [`RelayPolicy`] passed to [`Node::bind`] — there is deliberately no
//! variant of that policy that reaches iroh's public relay infrastructure,
//! so D4 ("never fall back to it") is a property of the type, not just a
//! convention callers have to follow. [`Invite`] is the invite-only join
//! credential (D12). [`ConnectivityReport`] is the first-run CGNAT
//! self-test (D13).

mod connectivity;
mod error;
mod invite;
mod node;
mod protocol;
mod relay;

pub use connectivity::ConnectivityReport;
pub use error::NetError;
pub use invite::Invite;
// Re-exported rather than leaving callers to depend on `iroh` directly for
// it: `Node::addr()` returns it and `get_remote`/`put_remote` take it, so
// it's already part of this crate's public API surface either way.
pub use iroh::EndpointAddr;
pub use node::Node;
pub use protocol::ALPN;
pub use relay::RelayPolicy;
