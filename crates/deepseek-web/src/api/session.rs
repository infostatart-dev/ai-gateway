use serde_json::Value;

use crate::{
    Error,
    constants::{SESSION_CREATE_URL, SESSION_DELETE_URL},
    cookie::generate_fake_cookie,
    headers::json_headers_for_session,
    session::file::BrowserSession,
    tls::fetch::{FetchRequest, HttpFetch},
};

pub async fn create_session(
    fetch: &dyn HttpFetch,
    access_token: &str,
) -> Result<String, Error> {
    create_session_with_browser(fetch, access_token, None).await
}

pub async fn create_session_with_browser(
    fetch: &dyn HttpFetch,
    access_token: &str,
    session: Option<&BrowserSession>,
) -> Result<String, Error> {
    let mut headers = json_headers_for_session(access_token, session);
    if session.and_then(|s| s.cookie.as_deref()).is_none() {
        headers.push(("Cookie".into(), generate_fake_cookie()));
    }

    let resp = fetch
        .fetch(FetchRequest {
            url: SESSION_CREATE_URL.into(),
            method: "POST".into(),
            headers,
            body: Some(b"{}".to_vec()),
            timeout_ms: 30_000,
        })
        .await?;

    if resp.status >= 400 {
        return Err(Error::Upstream {
            status: resp.status,
            message: format!("chat_session/create HTTP {}", resp.status),
        });
    }

    session_id_from_json(&resp.body)
}

pub async fn delete_session(
    fetch: &dyn HttpFetch,
    access_token: &str,
    session_id: &str,
) {
    delete_session_with_browser(fetch, access_token, session_id, None).await;
}

pub async fn delete_session_with_browser(
    fetch: &dyn HttpFetch,
    access_token: &str,
    session_id: &str,
    session: Option<&BrowserSession>,
) {
    let body = serde_json::json!({ "chat_session_id": session_id }).to_string();
    let _ = fetch
        .fetch(FetchRequest {
            url: SESSION_DELETE_URL.into(),
            method: "POST".into(),
            headers: json_headers_for_session(access_token, session),
            body: Some(body.into_bytes()),
            timeout_ms: 15_000,
        })
        .await;
}

fn session_id_from_json(body: &[u8]) -> Result<String, Error> {
    let v: Value = serde_json::from_slice(body)?;
    let code = v.get("code").and_then(Value::as_i64);
    let id = v
        .pointer("/data/biz_data/chat_session/id")
        .or_else(|| v.pointer("/biz_data/chat_session/id"))
        .and_then(Value::as_str);
    id.map(str::to_string)
        .ok_or_else(|| Error::Other(format!("No session id: code={code:?}")))
}
