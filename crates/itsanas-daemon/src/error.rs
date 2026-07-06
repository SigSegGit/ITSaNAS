use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("no account has been set up yet")]
    NoAccount,
    #[error("an account already exists")]
    AccountExists,
    #[error("incorrect password")]
    WrongPassword,
    #[error("the vault is locked; unlock it first")]
    Locked,
    #[error("no file named {0:?}")]
    FileNotFound(String),
    #[error("storage error: {0}")]
    Storage(#[from] itsanas_storage::StorageError),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("(de)serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("corrupt vault data: {0}")]
    Corrupt(String),
}

impl IntoResponse for DaemonError {
    fn into_response(self) -> Response {
        let status = match &self {
            DaemonError::NoAccount => StatusCode::NOT_FOUND,
            DaemonError::AccountExists => StatusCode::CONFLICT,
            DaemonError::WrongPassword => StatusCode::UNAUTHORIZED,
            DaemonError::Locked => StatusCode::UNAUTHORIZED,
            DaemonError::FileNotFound(_) => StatusCode::NOT_FOUND,
            DaemonError::Storage(_) | DaemonError::Io(_) | DaemonError::Serde(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            DaemonError::Corrupt(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string()).into_response()
    }
}
