//! Content-defined chunking and BLAKE3 content addressing for ITSaNAS.
//!
//! M0 uses fixed-size chunking ([`chunker`]) rather than content-defined
//! chunking; see `ARCHITECTURE.md` for the rationale and upgrade path.
//! BLAKE3 content addressing ([`chunk_id`]) and verify-on-read
//! ([`verify`]) are the mechanisms D7 relies on to treat storage backends
//! as hostile/unreliable: a chunk's own id proves its content, whoever is
//! storing it.

pub mod chunk_id;
pub mod chunker;
pub mod verify;

pub use chunk_id::ChunkId;
pub use chunker::{chunk, Chunk, DEFAULT_CHUNK_SIZE};
pub use verify::{verify as verify_chunk, VerifyError};
