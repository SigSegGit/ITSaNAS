use std::fmt;

/// A BLAKE3 content address for a chunk of bytes (D10).
///
/// Two chunks with the same bytes always have the same `ChunkId`, and
/// (short of a BLAKE3 collision) two chunks with the same `ChunkId` always
/// have the same bytes — this is what makes storage backends safe to treat
/// as hostile (D7): a shard's own name proves its content on read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChunkId([u8; 32]);

impl ChunkId {
    /// Computes the `ChunkId` for a chunk's bytes.
    pub fn of(data: &[u8]) -> Self {
        ChunkId(*blake3::hash(data).as_bytes())
    }

    /// The raw 32-byte BLAKE3 hash.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ChunkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_bytes_produce_same_id() {
        assert_eq!(ChunkId::of(b"hello"), ChunkId::of(b"hello"));
    }

    #[test]
    fn different_bytes_produce_different_ids() {
        assert_ne!(ChunkId::of(b"hello"), ChunkId::of(b"world"));
    }

    #[test]
    fn empty_input_matches_known_blake3_test_vector() {
        // Official BLAKE3 test vector for zero-length input.
        let expected = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";
        assert_eq!(expected.len(), 64);
        assert_eq!(ChunkId::of(b"").to_string(), expected);
    }

    #[test]
    fn display_is_lowercase_hex_of_the_hash() {
        let id = ChunkId::of(b"hello");
        let text = id.to_string();
        assert_eq!(text.len(), 64);
        assert!(text
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
}
