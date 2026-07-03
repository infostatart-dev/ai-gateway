use serde_json::{Value, json};

use super::prompt::messages_to_prompt;
use crate::session::file::BrowserSession;

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub chat_session_id: String,
    pub model_type: String,
    pub prompt: String,
    pub ref_file_ids: Vec<Value>,
    pub thinking_enabled: bool,
    pub search_enabled: bool,
}

pub fn build_completion_from_prompt(
    body: &Value,
    model: &str,
    session_id: &str,
    prompt: String,
) -> CompletionRequest {
    let opts = super::model::resolve_model_options(model, body);
    let ref_file_ids = body
        .get("ref_file_ids")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    CompletionRequest {
        chat_session_id: session_id.to_string(),
        model_type: opts.model_type,
        prompt,
        ref_file_ids,
        thinking_enabled: opts.thinking_enabled,
        search_enabled: opts.search_enabled,
    }
}

pub fn build_completion_request(
    body: &Value,
    model: &str,
    session_id: &str,
    history_window: usize,
) -> CompletionRequest {
    let opts = super::model::resolve_model_options(model, body);
    let messages = body
        .get("messages")
        .and_then(Value::as_array)
        .map(|a| a.as_slice())
        .unwrap_or(&[]);
    let prompt = messages_to_prompt(messages, history_window);
    let ref_file_ids = body
        .get("ref_file_ids")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    CompletionRequest {
        chat_session_id: session_id.to_string(),
        model_type: opts.model_type,
        prompt,
        ref_file_ids,
        thinking_enabled: opts.thinking_enabled,
        search_enabled: opts.search_enabled,
    }
}

pub fn completion_json(req: &CompletionRequest) -> Value {
    json!({
        "chat_session_id": req.chat_session_id,
        "parent_message_id": null,
        "model_type": req.model_type,
        "prompt": req.prompt,
        "ref_file_ids": req.ref_file_ids,
        "thinking_enabled": req.thinking_enabled,
        "search_enabled": req.search_enabled,
        "preempt": false,
    })
}

pub fn completion_headers(
    access_token: &str,
    pow_response: &str,
) -> Vec<(String, String)> {
    completion_headers_for_session(access_token, pow_response, None)
}

pub fn completion_headers_for_session(
    access_token: &str,
    pow_response: &str,
    session: Option<&BrowserSession>,
) -> Vec<(String, String)> {
    let mut headers =
        crate::headers::json_headers_for_session(access_token, session);
    headers.push(("X-Ds-Pow-Response".into(), pow_response.to_string()));
    if session.and_then(|s| s.cookie.as_deref()).is_none() {
        headers.push(("Cookie".into(), crate::cookie::generate_fake_cookie()));
    }
    headers.push(("Accept".into(), "text/event-stream".into()));
    headers
}
