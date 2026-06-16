use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("session auth failed: {0}")]
    SessionAuth(String),
    #[error("upstream HTTP {status}: {message}")]
    Upstream { status: u16, message: String },
    #[error("empty response from DeepSeek")]
    EmptyResponse,
    #[error("missing session file path ({0})")]
    MissingSession(&'static str),
    #[error("TLS client error: {0}")]
    Tls(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}
