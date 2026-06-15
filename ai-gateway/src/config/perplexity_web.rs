use std::path::{Path, PathBuf};

pub const SESSION_ENV: &str = "PERPLEXITY_BROWSER_CLI";
pub const DEFAULT_CREDENTIAL_ID: &str = "perplexity-web-default";

pub fn session_path_from_env() -> Option<PathBuf> {
    std::env::var(SESSION_ENV).ok().map(PathBuf::from)
}

#[must_use]
pub fn session_file_available() -> bool {
    session_path_from_env().is_some_and(|p| session_valid(&p))
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

/// Session file path for a credential slot (`AI_GATEWAY_CREDENTIAL_<ID>`).
#[must_use]
pub fn session_path_for_credential(credential_id: &str) -> Option<PathBuf> {
    let from_slot =
        crate::config::credential_env::credential_env_var_name(credential_id);
    if let Ok(path) = std::env::var(&from_slot) {
        let path = PathBuf::from(path);
        if session_valid(&path) {
            return Some(path);
        }
    }
    if credential_id == DEFAULT_CREDENTIAL_ID {
        return session_path_from_env().filter(|p| session_valid(p));
    }
    None
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

    #[test]
    fn bare_token_session_file_normalizes() {
        let path = std::env::temp_dir().join("ai-gw-pplx-bare.json");
        write_session(&path, r#"{"cookie":"eyJhbGc"}"#);
        let cookie = load_session_cookie(&path).unwrap();
        assert!(cookie.contains("__Secure-next-auth.session-token=eyJhbGc"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    #[serial_test::serial(env)]
    fn session_path_for_default_credential_reads_env() {
        let path = std::env::temp_dir().join("ai-gw-pplx-cred.json");
        write_session(
            &path,
            r#"{"cookie":"__Secure-next-auth.session-token=tok"}"#,
        );
        let env_name = crate::config::credential_env::credential_env_var_name(
            DEFAULT_CREDENTIAL_ID,
        );
        unsafe {
            std::env::set_var(&env_name, &path);
        }
        let resolved =
            session_path_for_credential(DEFAULT_CREDENTIAL_ID).unwrap();
        assert_eq!(resolved, path);
        unsafe {
            std::env::remove_var(&env_name);
        }
        let _ = std::fs::remove_file(&path);
    }
}
