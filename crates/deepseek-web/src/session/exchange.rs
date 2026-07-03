use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use serde::Deserialize;

use crate::{
    Error,
    constants::{TOKEN_TTL_SECS, USERS_CURRENT_URL},
    headers::{auth_headers, auth_headers_for_session},
    session::file::BrowserSession,
    tls::fetch::{FetchRequest, HttpFetch},
};

#[derive(Debug, Clone)]
pub struct AccessToken {
    pub token: String,
    pub expires_at: Instant,
}

static TOKEN_CACHE: LazyLock<Mutex<HashMap<String, AccessToken>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Deserialize)]
struct UsersCurrentResponse {
    code: Option<i64>,
    msg: Option<String>,
    data: Option<ResponseData>,
    biz_data: Option<BizData>,
}

#[derive(Debug, Deserialize)]
struct ResponseData {
    biz_data: Option<BizData>,
    biz_msg: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BizData {
    token: Option<String>,
}

pub async fn exchange_session(
    fetch: &dyn HttpFetch,
    user_token: &str,
) -> Result<AccessToken, Error> {
    exchange_session_inner(fetch, user_token, None).await
}

pub async fn exchange_browser_session(
    fetch: &dyn HttpFetch,
    session: &BrowserSession,
) -> Result<AccessToken, Error> {
    exchange_session_inner(fetch, &session.token, Some(session)).await
}

async fn exchange_session_inner(
    fetch: &dyn HttpFetch,
    user_token: &str,
    session: Option<&BrowserSession>,
) -> Result<AccessToken, Error> {
    if let Some(entry) = token_lookup(user_token) {
        return Ok(entry);
    }

    let headers = session.map_or_else(
        || auth_headers(user_token),
        |session| auth_headers_for_session(user_token, Some(session)),
    );

    let resp = fetch
        .fetch(FetchRequest {
            url: USERS_CURRENT_URL.into(),
            method: "GET".into(),
            headers,
            body: None,
            timeout_ms: 30_000,
        })
        .await?;

    if resp.status == 401 || resp.status == 403 {
        invalidate_token_cache(user_token);
        return Err(Error::SessionAuth(
            "Token invalid or expired — get a new userToken from DeepSeek \
             localStorage"
                .into(),
        ));
    }
    if resp.status >= 400 {
        return Err(Error::Upstream {
            status: resp.status,
            message: format!("users/current HTTP {}", resp.status),
        });
    }

    let json: UsersCurrentResponse = serde_json::from_slice(&resp.body)?;
    if json.code.is_some_and(|c| c != 0) {
        invalidate_token_cache(user_token);
        let msg = json
            .msg
            .or_else(|| json.data.as_ref().and_then(|d| d.biz_msg.clone()))
            .unwrap_or_else(|| "DeepSeek rejected token".into());
        return Err(Error::SessionAuth(format!(
            "DeepSeek rejected token: {msg}"
        )));
    }

    let biz = json
        .data
        .and_then(|d| d.biz_data)
        .or(json.biz_data)
        .ok_or_else(|| Error::SessionAuth("missing biz_data".into()))?;
    let token = biz
        .token
        .ok_or_else(|| Error::SessionAuth("missing access token".into()))?;

    let entry = AccessToken {
        token,
        expires_at: Instant::now() + Duration::from_secs(TOKEN_TTL_SECS),
    };
    token_store(user_token, entry.clone());
    Ok(entry)
}

fn token_lookup(user_token: &str) -> Option<AccessToken> {
    let cache = TOKEN_CACHE.lock().ok()?;
    let entry = cache.get(user_token)?;
    if entry.expires_at > Instant::now() {
        Some(entry.clone())
    } else {
        None
    }
}

fn token_store(user_token: &str, entry: AccessToken) {
    if let Ok(mut cache) = TOKEN_CACHE.lock() {
        if cache.len() >= 100 {
            cache.clear();
        }
        cache.insert(user_token.to_string(), entry);
    }
}

pub fn invalidate_token_cache(user_token: &str) {
    if let Ok(mut cache) = TOKEN_CACHE.lock() {
        cache.remove(user_token);
    }
}

#[cfg(test)]
pub fn clear_token_cache() {
    if let Ok(mut cache) = TOKEN_CACHE.lock() {
        cache.clear();
    }
}
