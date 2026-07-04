use iroh::EndpointAddr;

/// Result of a first-run connectivity self-test (D13).
///
/// Built from the node's own [`EndpointAddr`] once it has finished
/// discovering candidates (see [`crate::Node::connectivity_report`], which
/// awaits `Endpoint::online()` first) — this only inspects addresses
/// already known to iroh, it doesn't itself probe anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectivityReport {
    /// How many direct (non-relay) IP addresses were discovered for this
    /// node.
    pub direct_addr_count: usize,
    /// Whether this node has a relay to fall back on.
    pub has_relay: bool,
}

impl ConnectivityReport {
    /// No direct address was discovered at all: every inbound connection
    /// to this node will have to go through the relay. This is the
    /// signature of CGNAT or an equally restrictive NAT/firewall.
    pub fn likely_cgnat(&self) -> bool {
        self.direct_addr_count == 0
    }
}

/// Builds a [`ConnectivityReport`] from a node's [`EndpointAddr`].
pub fn report_for(addr: &EndpointAddr) -> ConnectivityReport {
    ConnectivityReport {
        direct_addr_count: addr.ip_addrs().count(),
        has_relay: addr.relay_urls().next().is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::{EndpointId, RelayUrl, TransportAddr};

    fn endpoint_id() -> EndpointId {
        itsanas_test_key()
    }

    // A fixed, arbitrary-but-valid Ed25519 public key, just to construct an
    // EndpointAddr for these pure, offline unit tests.
    fn itsanas_test_key() -> EndpointId {
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
        let verifying_key = signing_key.verifying_key();
        EndpointId::from_bytes(verifying_key.as_bytes()).expect("valid key")
    }

    #[test]
    fn no_addresses_is_likely_cgnat() {
        let addr = EndpointAddr::new(endpoint_id());
        let report = report_for(&addr);
        assert_eq!(report.direct_addr_count, 0);
        assert!(!report.has_relay);
        assert!(report.likely_cgnat());
    }

    #[test]
    fn a_direct_address_is_not_cgnat() {
        let addr = EndpointAddr::from_parts(
            endpoint_id(),
            [TransportAddr::Ip("127.0.0.1:1234".parse().unwrap())],
        );
        let report = report_for(&addr);
        assert_eq!(report.direct_addr_count, 1);
        assert!(!report.likely_cgnat());
    }

    #[test]
    fn relay_only_is_likely_cgnat_but_has_relay() {
        let relay_url: RelayUrl = "https://relay.example.org".parse().unwrap();
        let addr = EndpointAddr::from_parts(endpoint_id(), [TransportAddr::Relay(relay_url)]);
        let report = report_for(&addr);
        assert_eq!(report.direct_addr_count, 0);
        assert!(report.has_relay);
        assert!(report.likely_cgnat());
    }
}
