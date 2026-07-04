use rand_core::{OsRng, RngCore};

use crate::cipher::{self, KEY_LEN};
use crate::error::CryptoError;

/// Generates a new random per-file content key.
///
/// Bulk data is always encrypted under a per-file key rather than the user's
/// master key directly; the per-file key is then wrapped (see [`wrap_key`])
/// so the master key itself never touches bulk ciphertext.
pub fn generate_file_key() -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut key);
    key
}

/// Wraps (encrypts) a per-file key under the user's master key.
pub fn wrap_key(master_key: &[u8; KEY_LEN], file_key: &[u8; KEY_LEN]) -> Vec<u8> {
    cipher::encrypt(master_key, file_key, b"itsanas-key-wrap")
}

/// Unwraps a per-file key previously produced by [`wrap_key`].
pub fn unwrap_key(
    master_key: &[u8; KEY_LEN],
    wrapped: &[u8],
) -> Result<[u8; KEY_LEN], CryptoError> {
    let unwrapped = cipher::decrypt(master_key, wrapped, b"itsanas-key-wrap")?;
    unwrapped
        .try_into()
        .map_err(|v: Vec<u8>| CryptoError::InvalidLength {
            expected: KEY_LEN,
            actual: v.len(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_and_unwrap_round_trips() {
        let master_key = [1u8; KEY_LEN];
        let file_key = generate_file_key();

        let wrapped = wrap_key(&master_key, &file_key);
        let unwrapped = unwrap_key(&master_key, &wrapped).unwrap();

        assert_eq!(unwrapped, file_key);
    }

    #[test]
    fn generated_file_keys_are_distinct() {
        let a = generate_file_key();
        let b = generate_file_key();
        assert_ne!(a, b);
    }

    #[test]
    fn unwrap_fails_under_wrong_master_key() {
        let file_key = generate_file_key();
        let wrapped = wrap_key(&[1u8; KEY_LEN], &file_key);
        assert!(unwrap_key(&[2u8; KEY_LEN], &wrapped).is_err());
    }
}
