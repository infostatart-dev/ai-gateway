use crate::headers::{browser_headers, oai_headers};
use crate::session::cookie::build_session_cookie_header;
use crate::tls::fetch::{FetchRequest, HttpFetch};

const WARMUP_URLS: &[&str] = &[
    "https://chatgpt.com/backend-api/me",
    "https://chatgpt.com/backend-api/conversations?offset=0&limit=28&order=updated",
    "https://chatgpt.com/backend-api/models?history_and_training_disabled=false",
];

/// Best-effort browser-like warmup before sentinel (OmniRoute `runSessionWarmup`).
pub async fn run_session_warmup(
    fetch: &dyn HttpFetch,
    access_token: &str,
    account_id: Option<&str>,
    session_id: &str,
    device_id: &str,
    cookie: &str,
) {
    let mut headers = browser_headers();
    headers.extend(oai_headers(session_id, device_id));
    headers.push(("Accept".into(), "*/*".into()));
    headers.push(("Authorization".into(), format!("Bearer {access_token}")));
    headers.push(("Cookie".into(), build_session_cookie_header(cookie)));
    if let Some(id) = account_id {
        headers.push(("chatgpt-account-id".into(), id.to_string()));
    }

    for url in WARMUP_URLS {
        let _ = fetch
            .fetch(FetchRequest {
                url: (*url).into(),
                method: "GET".into(),
                headers: headers.clone(),
                body: None,
                timeout_ms: 15_000,
            })
            .await;
    }
}
