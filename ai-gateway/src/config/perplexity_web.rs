use std::path::{Path, PathBuf};

pub const DEFAULT_SESSION_PATH: &str = "dev/perplexity-session.json";
pub const DEFAULT_CREDENTIAL_ID: &str = "perplexity-web-default";

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
    load_session_cookie(path).is_some()
}

#[must_use]
pub fn load_session_cookie(path: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let session: perplexity_web::SessionFile =
        serde_json::from_str(&raw).ok()?;
    let cookie = session.normalized_cookie();
    perplexity_web::session::cookie::has_session_token(&cookie)
        .then_some(cookie)
}

#[must_use]
pub fn session_path_for_credential(credential_id: &str) -> Option<PathBuf> {
    crate::config::secrets_file::SecretsFile::session_path(credential_id)
        .filter(|p| session_valid(p))
}

#[must_use]
pub fn is_perplexity_web(
    provider: &crate::types::provider::InferenceProvider,
) -> bool {
    matches!(
        provider,
        crate::types::provider::InferenceProvider::Named(name)
            if name.as_str() == "perplexity-web"
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn write_session(path: &Path, json: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, json).unwrap();
    }

    #[test]
    fn identifies_perplexity_web_provider() {
        assert!(is_perplexity_web(
            &crate::types::provider::InferenceProvider::Named(
                "perplexity-web".into()
            )
        ));
    }

    #[test]
    fn session_valid_rejects_cf_only_guest() {
        let path = std::env::temp_dir().join("ai-gw-pplx-guest.json");
        write_session(&path, r#"{"cookie":"cf_clearance=abc123"}"#);
        assert!(!session_valid(&path));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn session_valid_requires_login_token() {
        let path = std::env::temp_dir().join("ai-gw-pplx-valid.json");
        write_session(
            &path,
            r#"{"cookie":"__Secure-next-auth.session-token=abc123"}"#,
        );
        assert!(session_valid(&path));
        assert_eq!(
            load_session_cookie(&path).as_deref(),
            Some("__Secure-next-auth.session-token=abc123")
        );
        let _ = std::fs::remove_file(&path);
    }
}
