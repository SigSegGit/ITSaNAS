use itsanas_chunking::ChunkId;

use crate::error::NetError;

/// ALPN identifying ITSaNAS's shard request/response protocol on the QUIC
/// connection (D4).
pub const ALPN: &[u8] = b"itsanas/shard/0";

/// Upper bound on a single request or response message, generous enough
/// for one [`itsanas_chunking::DEFAULT_CHUNK_SIZE`] chunk plus a small
/// header.
pub const MAX_MESSAGE_SIZE: usize = 8 * 1024 * 1024;

/// A request sent over a single bidirectional QUIC stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Fetch the shard stored under this id.
    Get(ChunkId),
    /// Store this shard under this id.
    Put(ChunkId, Vec<u8>),
}

impl Request {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Request::Get(id) => {
                let mut buf = Vec::with_capacity(1 + 32);
                buf.push(0u8);
                buf.extend_from_slice(id.as_bytes());
                buf
            }
            Request::Put(id, data) => {
                let mut buf = Vec::with_capacity(1 + 32 + data.len());
                buf.push(1u8);
                buf.extend_from_slice(id.as_bytes());
                buf.extend_from_slice(data);
                buf
            }
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, NetError> {
        let (&tag, rest) = bytes
            .split_first()
            .ok_or_else(|| NetError::Protocol("empty request".to_string()))?;
        match tag {
            0 => Ok(Request::Get(decode_chunk_id(rest)?)),
            1 => {
                if rest.len() < 32 {
                    return Err(NetError::Protocol("put request too short".to_string()));
                }
                let (id_bytes, data) = rest.split_at(32);
                Ok(Request::Put(decode_chunk_id(id_bytes)?, data.to_vec()))
            }
            other => Err(NetError::Protocol(format!("unknown request tag {other}"))),
        }
    }
}

/// The response to a [`Request`], sent back over the same stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    /// No shard is stored under the requested id.
    NotFound,
    /// The requested shard's bytes.
    Found(Vec<u8>),
    /// A `Put` was stored successfully.
    Stored,
    /// The peer's storage layer failed to serve the request.
    Error(String),
}

impl Response {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Response::NotFound => vec![0u8],
            Response::Found(data) => {
                let mut buf = Vec::with_capacity(1 + data.len());
                buf.push(1u8);
                buf.extend_from_slice(data);
                buf
            }
            Response::Stored => vec![2u8],
            Response::Error(msg) => {
                let mut buf = Vec::with_capacity(1 + msg.len());
                buf.push(3u8);
                buf.extend_from_slice(msg.as_bytes());
                buf
            }
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, NetError> {
        let (&tag, rest) = bytes
            .split_first()
            .ok_or_else(|| NetError::Protocol("empty response".to_string()))?;
        match tag {
            0 => Ok(Response::NotFound),
            1 => Ok(Response::Found(rest.to_vec())),
            2 => Ok(Response::Stored),
            3 => Ok(Response::Error(String::from_utf8_lossy(rest).into_owned())),
            other => Err(NetError::Protocol(format!("unknown response tag {other}"))),
        }
    }
}

fn decode_chunk_id(bytes: &[u8]) -> Result<ChunkId, NetError> {
    let array: [u8; 32] = bytes
        .try_into()
        .map_err(|_| NetError::Protocol("expected a 32-byte chunk id".to_string()))?;
    Ok(ChunkId::from_bytes(array))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_request_round_trips_through_encode_decode() {
        let id = ChunkId::of(b"some chunk");
        let req = Request::Get(id);
        assert_eq!(Request::decode(&req.encode()).unwrap(), req);
    }

    #[test]
    fn put_request_round_trips_through_encode_decode() {
        let id = ChunkId::of(b"some chunk");
        let req = Request::Put(id, b"some chunk".to_vec());
        assert_eq!(Request::decode(&req.encode()).unwrap(), req);
    }

    #[test]
    fn put_request_with_empty_data_round_trips() {
        let id = ChunkId::of(b"");
        let req = Request::Put(id, Vec::new());
        assert_eq!(Request::decode(&req.encode()).unwrap(), req);
    }

    #[test]
    fn found_response_round_trips() {
        let resp = Response::Found(b"shard bytes".to_vec());
        assert_eq!(Response::decode(&resp.encode()).unwrap(), resp);
    }

    #[test]
    fn not_found_response_round_trips() {
        assert_eq!(
            Response::decode(&Response::NotFound.encode()).unwrap(),
            Response::NotFound
        );
    }

    #[test]
    fn stored_response_round_trips() {
        assert_eq!(
            Response::decode(&Response::Stored.encode()).unwrap(),
            Response::Stored
        );
    }

    #[test]
    fn error_response_round_trips() {
        let resp = Response::Error("boom".to_string());
        assert_eq!(Response::decode(&resp.encode()).unwrap(), resp);
    }

    #[test]
    fn decoding_empty_request_fails() {
        assert!(Request::decode(&[]).is_err());
    }

    #[test]
    fn decoding_unknown_request_tag_fails() {
        assert!(Request::decode(&[99]).is_err());
    }

    #[test]
    fn decoding_truncated_put_request_fails() {
        // tag + only 5 bytes of what should be a 32-byte chunk id
        assert!(Request::decode(&[1, 1, 2, 3, 4, 5]).is_err());
    }
}
