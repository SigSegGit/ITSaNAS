use crate::chunk_id::ChunkId;

/// Default fixed chunk size: 1 MiB.
///
/// See `ARCHITECTURE.md` for why M0 uses fixed-size chunking rather than
/// content-defined chunking.
pub const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024;

/// A chunk of data together with its content address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub id: ChunkId,
    pub data: Vec<u8>,
}

/// Splits `data` into fixed-size chunks (the last chunk may be shorter),
/// computing each chunk's [`ChunkId`] as it goes.
///
/// Splitting an empty input yields no chunks.
pub fn chunk(data: &[u8], chunk_size: usize) -> Vec<Chunk> {
    assert!(chunk_size > 0, "chunk_size must be positive");
    data.chunks(chunk_size)
        .map(|slice| Chunk {
            id: ChunkId::of(slice),
            data: slice.to_vec(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_yields_no_chunks() {
        assert!(chunk(b"", DEFAULT_CHUNK_SIZE).is_empty());
    }

    #[test]
    fn input_smaller_than_chunk_size_yields_one_chunk() {
        let chunks = chunk(b"hello world", 1024);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].data, b"hello world");
        assert_eq!(chunks[0].id, ChunkId::of(b"hello world"));
    }

    #[test]
    fn input_splits_at_exact_chunk_boundaries() {
        let data = vec![0u8; 30];
        let chunks = chunk(&data, 10);
        assert_eq!(chunks.len(), 3);
        for c in &chunks {
            assert_eq!(c.data.len(), 10);
        }
    }

    #[test]
    fn last_chunk_may_be_shorter() {
        let data = vec![0u8; 25];
        let chunks = chunk(&data, 10);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].data.len(), 10);
        assert_eq!(chunks[1].data.len(), 10);
        assert_eq!(chunks[2].data.len(), 5);
    }

    #[test]
    fn reassembling_chunks_reproduces_the_original_data() {
        let data: Vec<u8> = (0..255u16).map(|n| (n % 256) as u8).collect();
        let chunks = chunk(&data, 17);
        let reassembled: Vec<u8> = chunks.iter().flat_map(|c| c.data.clone()).collect();
        assert_eq!(reassembled, data);
    }

    #[test]
    fn each_chunk_id_matches_its_own_data() {
        let data = vec![42u8; 100];
        for c in chunk(&data, 30) {
            assert_eq!(c.id, ChunkId::of(&c.data));
        }
    }
}
