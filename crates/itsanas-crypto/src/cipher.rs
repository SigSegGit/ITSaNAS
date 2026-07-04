use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};

use crate::error::CryptoError;

/// Length in bytes of an XChaCha20-Poly1305 key.
pub const KEY_LEN: usize = 32;

/// Length in bytes of an XChaCha20-Poly1305 nonce.
pub const NONCE_LEN: usize = 24;

/// Encrypts `plaintext` with XChaCha20-Poly1305 (D10) under `key`,
/// authenticating `aad` without encrypting it.
///
/// A fresh random nonce is generated per call and prepended to the returned
/// ciphertext, so the output of this function is self-contained: pass it
/// straight to [`decrypt`] with the same key and AAD. XChaCha20's 24-byte
/// nonce is large enough that random generation is safe without a counter,
/// which matters here since there is no central coordinator to hand out
/// sequential nonces across peers.
pub fn encrypt(key: &[u8; KEY_LEN], plaintext: &[u8], aad: &[u8]) -> Vec<u8> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            chacha20poly1305::aead::Payload {
                msg: plaintext,
                aad,
            },
        )
        .expect("encryption with a valid key and nonce cannot fail");

    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    out
}

/// Decrypts data produced by [`encrypt`] under the same `key` and `aad`.
///
/// Returns [`CryptoError::Aead`] if the data is too short to contain a
/// nonce, or if authentication fails (wrong key, wrong AAD, or the
/// ciphertext was tampered with) — these cases are deliberately not
/// distinguished.
pub fn decrypt(key: &[u8; KEY_LEN], data: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if data.len() < NONCE_LEN {
        return Err(CryptoError::Aead);
    }
    let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
    let nonce = XNonce::from_slice(nonce_bytes);

    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .decrypt(
            nonce,
            chacha20poly1305::aead::Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| CryptoError::Aead)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> [u8; KEY_LEN] {
        [7u8; KEY_LEN]
    }

    #[test]
    fn round_trips_plaintext() {
        let plaintext = b"the shard payload".to_vec();
        let ciphertext = encrypt(&key(), &plaintext, b"");
        let decrypted = decrypt(&key(), &ciphertext, b"").unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn round_trips_with_aad() {
        let plaintext = b"the shard payload".to_vec();
        let aad = b"chunk-id-1234";
        let ciphertext = encrypt(&key(), &plaintext, aad);
        let decrypted = decrypt(&key(), &ciphertext, aad).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn two_encryptions_use_different_nonces() {
        let plaintext = b"same message".to_vec();
        let a = encrypt(&key(), &plaintext, b"");
        let b = encrypt(&key(), &plaintext, b"");
        assert_ne!(a[..NONCE_LEN], b[..NONCE_LEN]);
        assert_ne!(a, b);
    }

    #[test]
    fn wrong_key_fails_to_decrypt() {
        let plaintext = b"secret".to_vec();
        let ciphertext = encrypt(&key(), &plaintext, b"");
        let wrong_key = [9u8; KEY_LEN];
        assert!(decrypt(&wrong_key, &ciphertext, b"").is_err());
    }

    #[test]
    fn wrong_aad_fails_to_decrypt() {
        let plaintext = b"secret".to_vec();
        let ciphertext = encrypt(&key(), &plaintext, b"correct-aad");
        assert!(decrypt(&key(), &ciphertext, b"wrong-aad").is_err());
    }

    #[test]
    fn tampered_ciphertext_fails_to_decrypt() {
        let plaintext = b"secret".to_vec();
        let mut ciphertext = encrypt(&key(), &plaintext, b"");
        let last = ciphertext.len() - 1;
        ciphertext[last] ^= 0xff;
        assert!(decrypt(&key(), &ciphertext, b"").is_err());
    }

    #[test]
    fn truncated_data_fails_to_decrypt() {
        assert!(decrypt(&key(), b"short", b"").is_err());
    }
}
