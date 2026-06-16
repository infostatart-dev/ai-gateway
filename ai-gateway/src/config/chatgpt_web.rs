use std::path::PathBuf;

pub const DEFAULT_SESSION_PATH: &str = "dev/session.json";
pub const DEFAULT_CREDENTIAL_ID: &str = "chatgpt-web-default";

#[must_use]
pub fn default_session_path() -> PathBuf {
    PathBuf::from(DEFAULT_SESSION_PATH)
}

#[must_use]
pub fn session_path_for_credential(credential_id: &str) -> Option<PathBuf> {
    crate::config::secrets_file::SecretsFile::session_path(credential_id)
        .filter(|p| p.exists())
}

#[must_use]
pub fn session_file_available() -> bool {
    session_path_for_credential(DEFAULT_CREDENTIAL_ID).is_some()
}

#[must_use]
pub fn load_session_cookie(path: &std::path::Path) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let session: chatgpt_web::SessionFile = serde_json::from_str(&raw).ok()?;
    Some(session.normalized_cookie())
}

/// True when the client sent `OpenAI` `response_format.type = json_schema`.
#[must_use]
pub fn request_requires_json_schema(body: &serde_json::Value) -> bool {
    body.get("response_format")
        .and_then(|rf| rf.get("type"))
        .and_then(serde_json::Value::as_str)
        == Some("json_schema")
}

#[must_use]
pub fn is_chatgpt_web(
    provider: &crate::types::provider::InferenceProvider,
) -> bool {
    matches!(
        provider,
        crate::types::provider::InferenceProvider::Named(name)
            if name.as_str() == "chatgpt-web"
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

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
        assert!(is_chatgpt_web(
            &crate::types::provider::InferenceProvider::Named(
                "chatgpt-web".into()
            )
        ));
    }
}
