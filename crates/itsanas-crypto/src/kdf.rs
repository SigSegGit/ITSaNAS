use argon2::{Algorithm, Argon2, Params, Version};
use rand_core::{OsRng, RngCore};

use crate::error::CryptoError;

/// Length in bytes of a derived master key.
pub const MASTER_KEY_LEN: usize = 32;

/// Length in bytes of the salt used for key derivation.
pub const SALT_LEN: usize = 16;

/// Argon2id parameters for password-based master key derivation.
///
/// These are explicit (not the crate defaults) so they are visible and
/// tunable in one place: 19 MiB memory, 2 iterations, 1 degree of
/// parallelism (OWASP-recommended minimums for Argon2id, chosen so key
/// derivation stays fast enough for interactive login on the lowest-spec
/// target hardware, a Raspberry Pi 4B).
fn params() -> Params {
    Params::new(19 * 1024, 2, 1, Some(MASTER_KEY_LEN))
        .expect("static Argon2id parameters are always valid")
}

/// Generates a random salt suitable for [`derive_master_key`].
pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    salt
}

/// Derives a 32-byte master key from a user password and salt using
/// Argon2id (D10).
///
/// The same password and salt always derive the same key; a different salt
/// (as returned by [`generate_salt`], stored alongside the account) derives
/// an unrelated key even for the same password.
pub fn derive_master_key(
    password: &[u8],
    salt: &[u8; SALT_LEN],
) -> Result<[u8; MASTER_KEY_LEN], CryptoError> {
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params());
    let mut key = [0u8; MASTER_KEY_LEN];
    argon2
        .hash_password_into(password, salt, &mut key)
        .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_password_and_salt_derive_same_key() {
        let salt = generate_salt();
        let key1 = derive_master_key(b"correct horse battery staple", &salt).unwrap();
        let key2 = derive_master_key(b"correct horse battery staple", &salt).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn different_salt_derives_different_key() {
        let salt1 = generate_salt();
        let salt2 = generate_salt();
        assert_ne!(salt1, salt2);

        let key1 = derive_master_key(b"same password", &salt1).unwrap();
        let key2 = derive_master_key(b"same password", &salt2).unwrap();
        assert_ne!(key1, key2);
    }

    #[test]
    fn different_password_derives_different_key() {
        let salt = generate_salt();
        let key1 = derive_master_key(b"password one", &salt).unwrap();
        let key2 = derive_master_key(b"password two", &salt).unwrap();
        assert_ne!(key1, key2);
    }
}
