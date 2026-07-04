use std::sync::Arc;

use iroh::endpoint::presets;
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayMode};
use itsanas_chunking::{verify_chunk, ChunkId};
use itsanas_storage::{StorageError, StorageRoot};

use crate::error::NetError;
use crate::protocol::{Request, Response, ALPN, MAX_MESSAGE_SIZE};

/// A peer on the ITSaNAS network: an iroh [`Endpoint`] serving shard
/// requests out of a local [`StorageRoot`].
///
/// M1 binds with relaying disabled (LAN-only, direct connections only);
/// NAT traversal via a self-hosted relay (D4, D5) is M2's job and will
/// become a `relay_mode` option here rather than a rewrite.
#[derive(Clone)]
pub struct Node {
    endpoint: Endpoint,
    storage: Arc<StorageRoot>,
}

impl Node {
    /// Binds a new node backed by `storage`.
    pub async fn bind(storage: StorageRoot) -> Result<Self, NetError> {
        let endpoint = Endpoint::builder(presets::N0)
            .alpns(vec![ALPN.to_vec()])
            .relay_mode(RelayMode::Disabled)
            .bind()
            .await
            .map_err(|e| NetError::Transport(e.to_string()))?;
        Ok(Self {
            endpoint,
            storage: Arc::new(storage),
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

    let response = match request {
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

    send.write_all(&response.encode())
        .await
        .map_err(|e| NetError::Transport(e.to_string()))?;
    send.finish()
        .map_err(|e| NetError::Transport(e.to_string()))?;
    conn.closed().await;
    Ok(())
}
