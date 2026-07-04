use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use crate::{
    Error,
    constants::USER_LAST_USED_MODEL_CONFIG_URL,
    headers::{browser_headers, oai_headers},
    models::ThinkingEffort,
    session::cookie::{build_session_cookie_header, cookie_key},
    tls::fetch::{FetchRequest, HttpFetch},
};

const THINKING_EFFORT_TTL: Duration = Duration::from_secs(5 * 60);

static THINKING_EFFORT_CACHE: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct ThinkingEffortUpdate<'a> {
    pub model_slug: &'a str,
    pub effort: ThinkingEffort,
    pub access_token: &'a str,
    pub account_id: Option<&'a str>,
    pub session_id: &'a str,
    pub device_id: &'a str,
    pub cookie: &'a str,
}

pub async fn set_user_thinking_effort(
    fetch: &dyn HttpFetch,
    update: ThinkingEffortUpdate<'_>,
) -> Result<(), Error> {
    let cache_key = format!(
        "{}:{}:{}",
        cookie_key(update.cookie),
        update.model_slug,
        update.effort.as_str()
    );
    if let Ok(cache) = THINKING_EFFORT_CACHE.lock()
        && let Some(last) = cache.get(&cache_key)
        && last.elapsed() < THINKING_EFFORT_TTL
    {
        return Ok(());
    }

    let mut headers = browser_headers();
    headers.extend(oai_headers(update.session_id, update.device_id));
    headers.push(("Accept".into(), "application/json".into()));
    headers.push((
        "Authorization".into(),
        format!("Bearer {}", update.access_token),
    ));
    headers.push(("Cookie".into(), build_session_cookie_header(update.cookie)));
    headers.push(("Priority".into(), "u=4".into()));
    if let Some(id) = update.account_id {
        headers.push(("chatgpt-account-id".into(), id.to_string()));
    }

    let url = format!(
        "{USER_LAST_USED_MODEL_CONFIG_URL}?model_slug={}&thinking_effort={}",
        update.model_slug,
        update.effort.as_str()
    );
    let resp = fetch
        .fetch(FetchRequest {
            url,
            method: "PATCH".into(),
            headers,
            body: None,
            timeout_ms: 15_000,
        })
        .await?;

    if resp.status >= 400 {
        tracing::warn!(
            status = resp.status,
            model_slug = update.model_slug,
            effort = update.effort.as_str(),
            "chatgpt-web thinking_effort PATCH failed; continuing"
        );
        return Ok(());
    }

    if let Ok(mut cache) = THINKING_EFFORT_CACHE.lock() {
        if cache.len() >= 400 && !cache.contains_key(&cache_key) {
            cache.clear();
        }
        cache.insert(cache_key, Instant::now());
    }
    Ok(())
}

#[cfg(test)]
pub fn clear_thinking_effort_cache() {
    if let Ok(mut cache) = THINKING_EFFORT_CACHE.lock() {
        cache.clear();
    }
}
