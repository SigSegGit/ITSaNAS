use thiserror::Error;

/// Errors produced by `itsanas-crypto`.
#[derive(Debug, Error)]
pub enum CryptoError {
    /// Argon2id key derivation failed (e.g. invalid parameters).
    #[error("key derivation failed: {0}")]
    KeyDerivation(String),

    /// AEAD encryption or decryption failed. Decryption failures deliberately
    /// carry no detail beyond this, so callers can't distinguish "wrong key"
    /// from "tampered ciphertext" from a timing/error-message side channel.
    #[error("authenticated encryption/decryption failed")]
    Aead,

    /// A byte slice was the wrong length for the operation (e.g. a key or
    /// nonce of the wrong size).
    #[error("invalid input length: expected {expected}, got {actual}")]
    InvalidLength { expected: usize, actual: usize },
}
