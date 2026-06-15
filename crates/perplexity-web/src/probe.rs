//! Minimal SSE probe against Perplexity web API (session required).

use serde_json::{json, Value};

use crate::{
    constants::{API_VERSION, SSE_URL, USER_AGENT},
    session::cookie::has_session_token,
    tls::shared_client,
    Error,
};

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub status: u16,
    pub answer: String,
}

pub async fn probe_query(query: &str, cookie: &str) -> Result<ProbeResult, Error> {
    if !has_session_token(cookie) {
        return Err(Error::SessionAuth(
            "missing __Secure-next-auth.session-token — run perplexity login or import"
                .into(),
        ));
    }

    let query_json = json!({ "query": query }).to_string();
    let body = json!({
        "query_str": query_json,
        "params": {
            "query_str": query_json,
            "search_focus": "internet",
            "mode": "concise",
            "model_preference": "pplx_pro",
            "sources": ["web"],
            "attachments": [],
            "frontend_uuid": uuid::Uuid::new_v4().to_string(),
            "frontend_context_uuid": uuid::Uuid::new_v4().to_string(),
            "version": API_VERSION,
            "language": "en-US",
            "timezone": "UTC",
            "search_recency_filter": null,
            "is_incognito": true,
            "use_schematized_api": true,
            "last_backend_uuid": null
        }
    });

    let resp = shared_client()?
        .post(SSE_URL)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .header("Origin", "https://www.perplexity.ai")
        .header("Referer", "https://www.perplexity.ai/")
        .header("User-Agent", USER_AGENT)
        .header("X-App-ApiClient", "default")
        .header("X-App-ApiVersion", API_VERSION)
        .header("Cookie", cookie)
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::Tls(e.to_string()))?;
    let status = resp.status().as_u16();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| Error::Tls(e.to_string()))?;

    if status == 401 || status == 403 {
        let snippet = String::from_utf8_lossy(&bytes);
        return Err(Error::SessionAuth(format!(
            "HTTP {status}: {}",
            snippet.chars().take(200).collect::<String>()
        )));
    }
    if status >= 400 {
        return Err(Error::Upstream {
            status,
            message: String::from_utf8_lossy(&bytes).chars().take(300).collect(),
        });
    }

    let answer = extract_answer_from_sse(&bytes)?;
    if answer.trim().is_empty() {
        return Err(Error::EmptyResponse);
    }
    Ok(ProbeResult { status, answer })
}

fn extract_answer_from_sse(body: &[u8]) -> Result<String, Error> {
    let text = String::from_utf8_lossy(body);
    let mut full = String::new();
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with("data:") {
            continue;
        }
        let payload = line.trim_start_matches("data:").trim();
        if payload.is_empty() || payload == "[DONE]" {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(payload) else {
            continue;
        };
        if let Some(t) = event.get("text").and_then(Value::as_str)
            && t.len() > full.len()
        {
            full = t.to_string();
        }
        if let Some(blocks) = event.get("blocks").and_then(Value::as_array) {
            for block in blocks {
                let Some(mb) = block.get("markdown_block") else {
                    continue;
                };
                if let Some(chunks) = mb.get("chunks").and_then(Value::as_array) {
                    let joined: String =
                        chunks.iter().filter_map(Value::as_str).collect();
                    if joined.len() > full.len() {
                        full = joined;
                    }
                }
            }
        }
    }
    if let Some(clean) = extract_from_plan_json(&full) {
        full = clean;
    }
    Ok(full)
}

fn extract_from_plan_json(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('[') {
        return None;
    }
    let steps = serde_json::from_str::<Value>(trimmed).ok()?;
    let arr = steps.as_array()?;
    for step in arr {
        if step.get("step_type").and_then(Value::as_str) != Some("FINAL") {
            continue;
        }
        let answer_raw = step.pointer("/content/answer")?.as_str()?;
        let parsed = serde_json::from_str::<Value>(answer_raw).ok()?;
        return parsed
            .get("answer")
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_cookie_without_session_token() {
        let err = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(probe_query("hi", "cf_clearance=abc"))
            .unwrap_err();
        assert!(matches!(err, Error::SessionAuth(_)));
    }

    #[test]
    fn parses_final_step_json_blob() {
        let raw = r#"[{"step_type":"FINAL","content":{"answer":"{\"answer\":\"OK\"}"}}]"#;
        assert_eq!(extract_from_plan_json(raw).as_deref(), Some("OK"));
    }
}
