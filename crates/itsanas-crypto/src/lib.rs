//! Encryption, key derivation, identities, and key wrapping for ITSaNAS.
//!
//! Implements the cryptographic stack fixed by D10: Argon2id for
//! password-based key derivation ([`kdf`]), XChaCha20-Poly1305 for
//! authenticated encryption ([`cipher`]), Ed25519/X25519 for peer identity
//! and key exchange ([`identity`]), and per-file key wrapping under a user's
//! master key ([`wrap`]). Content addressing (BLAKE3) lives in
//! `itsanas-chunking` instead, since it's about data addressing rather than
//! confidentiality.
//!
//! Zero-knowledge is non-negotiable (D10): nothing in this crate encrypts or
//! derives keys in a way that a node operator who is not the data owner
//! could reverse.

pub mod cipher;
pub mod error;
pub mod identity;
pub mod kdf;
pub mod wrap;

pub use error::CryptoError;
