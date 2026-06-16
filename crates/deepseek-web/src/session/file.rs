use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    constants::SESSION_ENV, errors::Error, session::token::normalize_user_token,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionFile {
    pub token: String,
    #[serde(default = "Utc::now")]
    pub saved_at: DateTime<Utc>,
}

pub fn session_path_from_env() -> Option<PathBuf> {
    std::env::var(SESSION_ENV).ok().map(PathBuf::from)
}

pub async fn load_session(path: &Path) -> Result<SessionFile, Error> {
    if !path.exists() {
        return Err(Error::MissingSession(SESSION_ENV));
    }
    let raw = tokio::fs::read_to_string(path).await?;
    serde_json::from_str(&raw).map_err(Error::from)
}

pub async fn save_session(
    path: &Path,
    session: &SessionFile,
) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let normalized = SessionFile {
        token: normalize_user_token(&session.token),
        saved_at: session.saved_at,
    };
    let json = serde_json::to_string_pretty(&normalized)?;
    tokio::fs::write(path, json).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn roundtrip_session_file() {
        let path = std::env::temp_dir()
            .join(format!("deepseek-session-{}.json", std::process::id()));
        let session = SessionFile {
            token: "abc123".into(),
            saved_at: Utc::now(),
        };
        save_session(&path, &session).await.unwrap();
        let loaded = load_session(&path).await.unwrap();
        assert_eq!(loaded.token, "abc123");
        let _ = tokio::fs::remove_file(path).await;
    }
}
