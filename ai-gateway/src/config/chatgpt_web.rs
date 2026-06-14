use std::path::{Path, PathBuf};

pub const SESSION_ENV: &str = "CHATGPT_BROWSER_CLI";

pub fn session_path_from_env() -> Option<PathBuf> {
    std::env::var(SESSION_ENV).ok().map(PathBuf::from)
}

pub fn session_file_available() -> bool {
    session_path_from_env().is_some_and(|p| p.exists())
}

pub fn load_session_cookie() -> Option<String> {
    let path = session_path_from_env()?;
    let raw = std::fs::read_to_string(path).ok()?;
    let session: chatgpt_web::SessionFile = serde_json::from_str(&raw).ok()?;
    Some(session.normalized_cookie())
}

/// True when the client sent OpenAI `response_format.type = json_schema`.
#[must_use]
pub fn request_requires_json_schema(body: &serde_json::Value) -> bool {
    body.get("response_format")
        .and_then(|rf| rf.get("type"))
        .and_then(serde_json::Value::as_str)
        == Some("json_schema")
}

#[must_use]
pub fn is_chatgpt_web(provider: &crate::types::provider::InferenceProvider) -> bool {
    matches!(
        provider,
        crate::types::provider::InferenceProvider::Named(name) if name.as_str() == "chatgpt-web"
    )
}

pub fn session_path(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        Some(path.to_path_buf())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_json_schema_strict_request() {
        let body = json!({
            "model": "gpt-5-mini",
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "out",
                    "strict": true,
                    "schema": { "type": "object" }
                }
            },
            "messages": [{ "role": "user", "content": "hi" }]
        });
        assert!(request_requires_json_schema(&body));
    }

    #[test]
    fn json_object_does_not_require_schema_validation() {
        let body = json!({
            "response_format": { "type": "json_object" },
            "messages": []
        });
        assert!(!request_requires_json_schema(&body));
    }

    #[test]
    fn identifies_chatgpt_web_provider() {
        assert!(is_chatgpt_web(&crate::types::provider::InferenceProvider::Named(
            "chatgpt-web".into()
        )));
    }
}
