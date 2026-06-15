use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("session auth failed: {0}")]
    SessionAuth(String),
    #[error("upstream HTTP {status}: {message}")]
    Upstream { status: u16, message: String },
    #[error("missing session file (set {SESSION_ENV})")]
    MissingSession,
    #[error("empty response from Perplexity")]
    EmptyResponse,
    #[error("TLS client error: {0}")]
    Tls(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

use crate::constants::SESSION_ENV;
