use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    Error,
    constants::SESSION_ENV,
    session::cookie::{build_session_cookie_header, normalize_cookie_blob},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionFile {
    pub cookie: String,
    #[serde(default = "Utc::now")]
    pub saved_at: DateTime<Utc>,
}

impl SessionFile {
    #[must_use]
    pub fn normalized_cookie(&self) -> String {
        build_session_cookie_header(&self.cookie)
    }
}

#[must_use]
pub fn session_path_from_env() -> Option<PathBuf> {
    std::env::var(SESSION_ENV).ok().map(PathBuf::from)
}

pub async fn load_session(path: &Path) -> Result<SessionFile, Error> {
    if !path.exists() {
        return Err(Error::MissingSession);
    }
    let raw = tokio::fs::read_to_string(path).await?;
    serde_json::from_str(&raw).map_err(Error::from)
}

pub async fn save_session(path: &Path, session: &SessionFile) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let normalized = SessionFile {
        cookie: normalize_cookie_blob(&session.cookie),
        saved_at: session.saved_at,
    };
    let json = serde_json::to_string_pretty(&normalized)?;
    tokio::fs::write(path, json).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    #[tokio::test]
    async fn roundtrip_normalizes_cookie() {
        let path = std::env::temp_dir().join("pplx-roundtrip.json");
        let session = SessionFile {
            cookie: "eyJhbGc".into(),
            saved_at: Utc::now(),
        };
        save_session(&path, &session).await.unwrap();
        let loaded = load_session(&path).await.unwrap();
        assert!(loaded
            .normalized_cookie()
            .contains("__Secure-next-auth.session-token="));
        let _ = tokio::fs::remove_file(&path).await;
    }
}
