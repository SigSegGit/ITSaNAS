//! Single-user account: a password-derived master key (D10), with a
//! verification tag so a wrong password is rejected cleanly instead of
//! silently producing a useless key.

use std::path::Path;

use itsanas_crypto::cipher;
use itsanas_crypto::kdf::{self, SALT_LEN};
use serde::{Deserialize, Serialize};

use crate::error::DaemonError;
use crate::hex;

const VERIFY_PLAINTEXT: &[u8] = b"itsanas-account-verify-v1";
const ACCOUNT_FILE: &str = "account.json";

#[derive(Serialize, Deserialize)]
struct AccountFile {
    salt: String,
    verify: String,
}

/// Whether an account has already been set up in `data_dir`.
pub fn exists(data_dir: &Path) -> bool {
    data_dir.join(ACCOUNT_FILE).exists()
}

/// Creates a new account secured by `password`, returning its master key.
pub fn setup(data_dir: &Path, password: &str) -> Result<[u8; 32], DaemonError> {
    if exists(data_dir) {
        return Err(DaemonError::AccountExists);
    }
    let salt = kdf::generate_salt();
    let master_key = kdf::derive_master_key(password.as_bytes(), &salt)
        .map_err(|e| DaemonError::Corrupt(e.to_string()))?;
    let verify = cipher::encrypt(&master_key, VERIFY_PLAINTEXT, b"itsanas-account-verify");

    let file = AccountFile {
        salt: hex::encode(salt),
        verify: hex::encode(verify),
    };
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(data_dir.join(ACCOUNT_FILE), serde_json::to_vec(&file)?)?;
    Ok(master_key)
}

/// Derives the master key from `password` and checks it against the stored
/// verification tag, returning [`DaemonError::WrongPassword`] on mismatch.
pub fn unlock(data_dir: &Path, password: &str) -> Result<[u8; 32], DaemonError> {
    let raw = std::fs::read(data_dir.join(ACCOUNT_FILE)).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            DaemonError::NoAccount
        } else {
            DaemonError::Io(e)
        }
    })?;
    let file: AccountFile = serde_json::from_slice(&raw)?;

    let salt_bytes = hex::decode(&file.salt).map_err(|e| DaemonError::Corrupt(e.to_string()))?;
    let salt: [u8; SALT_LEN] = salt_bytes
        .try_into()
        .map_err(|_| DaemonError::Corrupt("stored salt has the wrong length".to_string()))?;
    let verify = hex::decode(&file.verify).map_err(|e| DaemonError::Corrupt(e.to_string()))?;

    let master_key = kdf::derive_master_key(password.as_bytes(), &salt)
        .map_err(|e| DaemonError::Corrupt(e.to_string()))?;

    match cipher::decrypt(&master_key, &verify, b"itsanas-account-verify") {
        Ok(plaintext) if plaintext == VERIFY_PLAINTEXT => Ok(master_key),
        _ => Err(DaemonError::WrongPassword),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_then_unlock_with_correct_password_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let key = setup(dir.path(), "correct horse battery staple").unwrap();
        let unlocked = unlock(dir.path(), "correct horse battery staple").unwrap();
        assert_eq!(key, unlocked);
    }

    #[test]
    fn unlock_with_wrong_password_fails() {
        let dir = tempfile::tempdir().unwrap();
        setup(dir.path(), "correct horse battery staple").unwrap();
        assert!(matches!(
            unlock(dir.path(), "wrong password"),
            Err(DaemonError::WrongPassword)
        ));
    }

    #[test]
    fn setup_twice_fails() {
        let dir = tempfile::tempdir().unwrap();
        setup(dir.path(), "one").unwrap();
        assert!(matches!(
            setup(dir.path(), "two"),
            Err(DaemonError::AccountExists)
        ));
    }

    #[test]
    fn unlock_without_setup_fails() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            unlock(dir.path(), "anything"),
            Err(DaemonError::NoAccount)
        ));
    }
}
