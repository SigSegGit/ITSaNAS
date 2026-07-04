use std::sync::Arc;

use iroh::endpoint::presets;
use iroh::{Endpoint, EndpointAddr, EndpointId};
use itsanas_chunking::{verify_chunk, ChunkId};
use itsanas_storage::{StorageError, StorageRoot};

use crate::connectivity::{report_for, ConnectivityReport};
use crate::error::NetError;
use crate::protocol::{Request, Response, ALPN, MAX_MESSAGE_SIZE};
use crate::relay::RelayPolicy;

/// A peer on the ITSaNAS network: an iroh [`Endpoint`] serving shard
/// requests out of a local [`StorageRoot`].
///
/// Direct LAN connections (M1) and relayed connections via a self-hosted
/// relay (M2, D4/D5) are both just a [`RelayPolicy`] passed to
/// [`Node::bind`] — there is no separate code path for either.
#[derive(Clone)]
pub struct Node {
    endpoint: Endpoint,
    storage: Arc<StorageRoot>,
    has_relay: bool,
}

impl Node {
    /// Binds a new node backed by `storage`, using `relay` for NAT
    /// traversal (D4, D5). Pass [`RelayPolicy::Disabled`] for LAN-only
    /// connectivity.
    ///
    /// Deliberately uses iroh's `Minimal` preset, not `N0`: `N0` bundles a
    /// DNS/pkarr address-lookup service that publishes to and resolves
    /// from n0.computer's own servers over the public internet. This
    /// project runs its own bootstrap/rendezvous infrastructure (D5) and
    /// must never depend on third-party discovery infrastructure any more
    /// than it depends on iroh's public relays (D4) — `Minimal` sets only
    /// the mandatory crypto provider and nothing else.
    pub async fn bind(storage: StorageRoot, relay: RelayPolicy) -> Result<Self, NetError> {
        let has_relay = !matches!(relay, RelayPolicy::Disabled);
        let endpoint = Endpoint::builder(presets::Minimal)
            .alpns(vec![ALPN.to_vec()])
            .relay_mode(relay.into_relay_mode())
            .bind()
            .await
            .map_err(|e| NetError::Transport(e.to_string()))?;
        Ok(Self {
            endpoint,
            storage: Arc::new(storage),
            has_relay,
        })
    }

    /// This node's address, to be shared with a peer so it can [`connect`](Self::get_remote).
    pub fn addr(&self) -> EndpointAddr {
        self.endpoint.addr()
    }

    /// This node's identity.
    pub fn id(&self) -> EndpointId {
        self.endpoint.id()
    }

    /// Runs the first-run connectivity self-test (D13): waits for the
    /// endpoint to finish discovering its own reachable addresses, then
    /// reports whether it has any direct (non-relay) address or looks
    /// like it's behind CGNAT / an equally restrictive NAT.
    ///
    /// Only waits on relay connectivity when a relay is actually
    /// configured (`Endpoint::online()` waits specifically for a relay
    /// connection to succeed, so calling it with `RelayPolicy::Disabled`
    /// would hang forever waiting for a relay that will never exist).
    pub async fn connectivity_report(&self) -> ConnectivityReport {
        if self.has_relay {
            self.endpoint.online().await;
        }
        report_for(&self.endpoint.addr())
    }

    /// Stores `data` directly in this node's own local storage, without
    /// going over the network. This is how a node seeds data it owns
    /// before ever serving it to a peer.
    pub fn put_local(&self, id: &ChunkId, data: &[u8]) -> Result<(), NetError> {
        Ok(self.storage.put(id, data)?)
    }

    /// Reads a shard directly from this node's own local storage, without
    /// going over the network.
    pub fn get_local(&self, id: &ChunkId) -> Result<Vec<u8>, NetError> {
        Ok(self.storage.get(id)?)
    }

    /// Serves shard requests from peers until the endpoint is closed.
    /// Intended to run as a background task, e.g.
    /// `tokio::spawn(node.clone().serve())`.
    pub async fn serve(self) -> Result<(), NetError> {
        while let Some(incoming) = self.endpoint.accept().await {
            let storage = self.storage.clone();
            tokio::spawn(async move {
                if let Err(err) = handle_connection(incoming, storage).await {
                    eprintln!("itsanas-net: connection handler error: {err}");
                }
            });
        }
        Ok(())
    }

    /// Fetches the shard `id` from `peer`, verifying its content address on
    /// receipt (D7's hostile-backend assumption extends to peers: the
    /// network is not a trust boundary either).
    pub async fn get_remote(&self, peer: EndpointAddr, id: &ChunkId) -> Result<Vec<u8>, NetError> {
        let response_bytes = self.request(peer, &Request::Get(*id)).await?;
        match Response::decode(&response_bytes)? {
            Response::Found(data) => {
                verify_chunk(&data, id)?;
                Ok(data)
            }
            Response::NotFound => Err(NetError::NotFound),
            Response::Error(msg) => Err(NetError::Remote(msg)),
            Response::Stored => Err(NetError::Protocol(
                "unexpected Stored response to a Get request".to_string(),
            )),
        }
    }

    /// Stores `data` under `id` on `peer`.
    pub async fn put_remote(
        &self,
        peer: EndpointAddr,
        id: &ChunkId,
        data: &[u8],
    ) -> Result<(), NetError> {
        let response_bytes = self
            .request(peer, &Request::Put(*id, data.to_vec()))
            .await?;
        match Response::decode(&response_bytes)? {
            Response::Stored => Ok(()),
            Response::Error(msg) => Err(NetError::Remote(msg)),
            _ => Err(NetError::Protocol(
                "unexpected response to a Put request".to_string(),
            )),
        }
    }

    /// Opens a connection to `peer`, sends `request`, and returns the raw
    /// response bytes.
    async fn request(&self, peer: EndpointAddr, request: &Request) -> Result<Vec<u8>, NetError> {
        let conn = self
            .endpoint
            .connect(peer, ALPN)
            .await
            .map_err(|e| NetError::Transport(e.to_string()))?;
        let (mut send, mut recv) = conn
            .open_bi()
            .await
            .map_err(|e| NetError::Transport(e.to_string()))?;

        send.write_all(&request.encode())
            .await
            .map_err(|e| NetError::Transport(e.to_string()))?;
        send.finish()
            .map_err(|e| NetError::Transport(e.to_string()))?;

        let response_bytes = recv
            .read_to_end(MAX_MESSAGE_SIZE)
            .await
            .map_err(|e| NetError::Transport(e.to_string()))?;
        conn.close(0u32.into(), b"done");

        Ok(response_bytes)
    }
}

async fn handle_connection(
    incoming: iroh::endpoint::Incoming,
    storage: Arc<StorageRoot>,
) -> Result<(), NetError> {
    let conn = incoming
        .await
        .map_err(|e| NetError::Transport(e.to_string()))?;
    let (mut send, mut recv) = conn
        .accept_bi()
        .await
        .map_err(|e| NetError::Transport(e.to_string()))?;

    let request_bytes = recv
        .read_to_end(MAX_MESSAGE_SIZE)
        .await
        .map_err(|e| NetError::Transport(e.to_string()))?;
    let request = Request::decode(&request_bytes)?;

    #[cfg_attr(not(feature = "test-mode"), allow(unused_mut))]
    let mut response = match request {
        Request::Get(id) => match storage.get(&id) {
            Ok(data) => Response::Found(data),
            Err(StorageError::NotFound(_)) => Response::NotFound,
            Err(e) => Response::Error(e.to_string()),
        },
        Request::Put(id, data) => match storage.put(&id, &data) {
            Ok(()) => Response::Stored,
            Err(e) => Response::Error(e.to_string()),
        },
    };

    #[cfg(feature = "test-mode")]
    if itsanas_testkit::should_fail(itsanas_testkit::FaultPoint::NetPeerDisconnectMidTransfer) {
        // Simulate this peer vanishing before it responds: drop the
        // connection instead of sending anything back. The requester must
        // surface this as a transport error rather than hanging.
        return Ok(());
    }

    #[cfg(feature = "test-mode")]
    if let Response::Found(data) = &mut response {
        if itsanas_testkit::should_fail(itsanas_testkit::FaultPoint::NetShardTamperInTransit) {
            // Simulate the shard being tampered with in transit, after
            // being read from (trustworthy) local storage. The requester's
            // content-address re-verification on receipt must catch this.
            if let Some(byte) = data.first_mut() {
                *byte ^= 0xff;
            }
        }
    }

    send.write_all(&response.encode())
        .await
        .map_err(|e| NetError::Transport(e.to_string()))?;
    send.finish()
        .map_err(|e| NetError::Transport(e.to_string()))?;
    conn.closed().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! M2 acceptance test: two nodes exchange a shard using only a
    //! self-hosted relay (D4, D5) for their initial contact information —
    //! never iroh's public relay infrastructure, and never a direct IP
    //! address handed to the connecting side out of band.
    //!
    //! The relay server here is `iroh-relay`'s real server implementation
    //! (via iroh's `test_utils::run_relay_server`), run in-process — the
    //! same code this project deploys to the Freebox VM (D5), not a mock
    //! of it. It uses a self-signed certificate, so (test-only!) this
    //! binds with `CaTlsConfig::insecure_skip_verify()` — never exposed
    //! through the public `Node::bind` API, since a real deployment
    //! always has a properly-signed relay certificate (see
    //! `scripts/relay/relay.example.toml`'s ACME config).
    //!
    //! Both nodes run on localhost, so iroh may additionally succeed at
    //! hole-punching a direct path once the relay has done its job —
    //! that's expected and fine. What this test actually proves is the
    //! property that matters for NAT traversal: an `EndpointAddr`
    //! containing *only* a relay URL (no direct IP candidates) is
    //! sufficient to establish a full connection and complete a real
    //! exchange, exactly as it would be for a peer this node can't
    //! otherwise reach directly.
    use iroh::tls::CaTlsConfig;
    use iroh::TransportAddr;

    use super::*;

    async fn bind_trusting_test_relay(
        storage: StorageRoot,
        relay: RelayPolicy,
    ) -> Result<Node, NetError> {
        let has_relay = !matches!(relay, RelayPolicy::Disabled);
        let endpoint = Endpoint::builder(presets::Minimal)
            .alpns(vec![ALPN.to_vec()])
            .relay_mode(relay.into_relay_mode())
            .ca_tls_config(CaTlsConfig::insecure_skip_verify())
            .bind()
            .await
            .map_err(|e| NetError::Transport(e.to_string()))?;
        Ok(Node {
            endpoint,
            storage: Arc::new(storage),
            has_relay,
        })
    }

    #[tokio::test]
    async fn two_nodes_exchange_a_shard_via_a_self_hosted_relay_only() {
        let (_relay_map, relay_url, _relay_server) = iroh::test_utils::run_relay_server()
            .await
            .expect("failed to start test relay server");

        let owner_dir = tempfile::tempdir().unwrap();
        let mirror_dir = tempfile::tempdir().unwrap();

        let relay_policy = || RelayPolicy::SelfHosted {
            url: relay_url.clone(),
            auth_token: None,
        };
        let owner =
            bind_trusting_test_relay(StorageRoot::open(owner_dir.path()).unwrap(), relay_policy())
                .await
                .unwrap();
        let mirror = bind_trusting_test_relay(
            StorageRoot::open(mirror_dir.path()).unwrap(),
            relay_policy(),
        )
        .await
        .unwrap();

        // Let both endpoints finish registering with the relay before
        // using it (this is the D13 self-test's online() wait, reused
        // here).
        let mirror_report = mirror.connectivity_report().await;
        let _owner_report = owner.connectivity_report().await;
        assert!(
            mirror_report.has_relay,
            "mirror should have registered a relay"
        );

        // Deliberately construct a relay-only address: no direct IP
        // candidates at all, only the relay URL and the mirror's
        // identity — exactly what this node would have for a peer it
        // can't reach directly.
        let mirror_relay_only_addr =
            EndpointAddr::from_parts(mirror.id(), [TransportAddr::Relay(relay_url)]);

        tokio::spawn(mirror.clone().serve());

        let data = b"a shard reached only through the relay";
        let id = ChunkId::of(data);
        owner
            .put_remote(mirror_relay_only_addr.clone(), &id, data)
            .await
            .expect("put over relay-only address should succeed");

        assert_eq!(mirror.get_local(&id).unwrap(), data);

        let fetched = owner
            .get_remote(mirror_relay_only_addr, &id)
            .await
            .expect("get over relay-only address should succeed");
        assert_eq!(fetched, data);
    }
}
