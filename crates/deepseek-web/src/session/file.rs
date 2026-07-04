use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    constants::SESSION_ENV, errors::Error, session::token::normalize_user_token,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionFile {
    pub token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cookie: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    #[serde(default = "Utc::now")]
    pub saved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserSession {
    pub token: String,
    pub cookie: Option<String>,
    pub headers: BTreeMap<String, String>,
}

impl BrowserSession {
    #[must_use]
    pub fn from_token(token: impl Into<String>) -> Self {
        Self {
            token: normalize_user_token(&token.into()),
            cookie: None,
            headers: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    #[must_use]
    pub fn has_browser_context(&self) -> bool {
        self.cookie.as_deref().is_some_and(|v| !v.trim().is_empty())
            || !self.headers.is_empty()
    }
}

impl From<&SessionFile> for BrowserSession {
    fn from(session: &SessionFile) -> Self {
        Self {
            token: normalize_user_token(&session.token),
            cookie: session.cookie.clone().filter(|v| !v.trim().is_empty()),
            headers: session.headers.clone(),
        }
    }
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
        cookie: session.cookie.clone(),
        headers: session.headers.clone(),
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
            cookie: Some("ds_session_id=session".into()),
            headers: BTreeMap::from([(
                "x-client-version".into(),
                "2.0.0".into(),
            )]),
            saved_at: Utc::now(),
        };
        save_session(&path, &session).await.unwrap();
        let loaded = load_session(&path).await.unwrap();
        assert_eq!(loaded.token, "abc123");
        assert_eq!(loaded.cookie.as_deref(), Some("ds_session_id=session"));
        assert_eq!(
            loaded.headers.get("x-client-version").map(String::as_str),
            Some("2.0.0")
        );
        let _ = tokio::fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn old_token_only_session_file_still_loads() {
        let path = std::env::temp_dir().join(format!(
            "deepseek-token-only-session-{}.json",
            std::process::id()
        ));
        tokio::fs::write(&path, r#"{"token":"abc123"}"#)
            .await
            .unwrap();
        let loaded = load_session(&path).await.unwrap();
        assert_eq!(loaded.token, "abc123");
        assert!(loaded.cookie.is_none());
        assert!(loaded.headers.is_empty());
        let _ = tokio::fs::remove_file(path).await;
    }
}
