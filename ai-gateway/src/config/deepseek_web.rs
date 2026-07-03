use std::path::{Path, PathBuf};

pub const DEFAULT_SESSION_PATH: &str = "dev/deepseek-session.json";
pub const DEFAULT_CREDENTIAL_ID: &str = "deepseek-web-default";

#[must_use]
pub fn default_session_path() -> PathBuf {
    PathBuf::from(DEFAULT_SESSION_PATH)
}

#[must_use]
pub fn session_file_available() -> bool {
    session_path_for_credential(DEFAULT_CREDENTIAL_ID).is_some()
}

#[must_use]
pub fn session_valid(path: &Path) -> bool {
    load_session_token(path).is_some()
}

#[must_use]
pub fn load_session_token(path: &Path) -> Option<String> {
    load_browser_session(path).map(|session| session.token)
}

#[must_use]
pub fn load_browser_session(
    path: &Path,
) -> Option<deepseek_web::BrowserSession> {
    let raw = std::fs::read_to_string(path).ok()?;
    let session: deepseek_web::SessionFile = serde_json::from_str(&raw).ok()?;
    let token = session.token.trim();
    (!token.is_empty()).then(|| deepseek_web::BrowserSession::from(&session))
}

#[must_use]
pub fn session_path_for_credential(credential_id: &str) -> Option<PathBuf> {
    crate::config::secrets_file::SecretsFile::session_path(credential_id)
        .filter(|p| session_valid(p))
}

#[must_use]
pub fn is_deepseek_web(
    provider: &crate::types::provider::InferenceProvider,
) -> bool {
    matches!(
        provider,
        crate::types::provider::InferenceProvider::Named(name)
            if name.as_str() == "deepseek-web"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_session(path: &Path, json: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, json).unwrap();
    }

    #[test]
    fn identifies_deepseek_web_provider() {
        assert!(is_deepseek_web(
            &crate::types::provider::InferenceProvider::Named(
                "deepseek-web".into()
            )
        ));
    }

    #[test]
    fn session_valid_requires_non_empty_token() {
        let path = std::env::temp_dir().join("ai-gw-ds-empty.json");
        write_session(&path, r#"{"token":""}"#);
        assert!(!session_valid(&path));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn session_valid_accepts_plain_token() {
        let path = std::env::temp_dir().join("ai-gw-ds-valid.json");
        write_session(&path, r#"{"token":"user-tok-abc"}"#);
        assert!(session_valid(&path));
        assert_eq!(load_session_token(&path).as_deref(), Some("user-tok-abc"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn loads_browser_session_bundle() {
        let path = std::env::temp_dir().join("ai-gw-ds-bundle.json");
        write_session(
            &path,
            r#"{
                "token":"user-tok-abc",
                "cookie":"ds_session_id=session",
                "headers":{"x-client-version":"2.0.0"}
            }"#,
        );
        let session = load_browser_session(&path).expect("browser session");
        assert_eq!(session.token, "user-tok-abc");
        assert_eq!(session.cookie.as_deref(), Some("ds_session_id=session"));
        assert_eq!(session.header("X-Client-Version"), Some("2.0.0"));
        let _ = std::fs::remove_file(&path);
    }
}
