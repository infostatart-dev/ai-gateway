use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use chrono::DateTime;
use serde::Deserialize;

use crate::{
    Error,
    constants::{SESSION_URL, TOKEN_TTL_MS},
    headers::browser_headers,
    session::cookie::{
        build_session_cookie_header, cookie_key, merge_refreshed_cookie,
    },
    tls::fetch::{FetchRequest, HttpFetch},
};

#[derive(Debug, Clone)]
pub struct TokenEntry {
    pub access_token: String,
    pub account_id: Option<String>,
    pub expires_at: Instant,
    pub refreshed_cookie: Option<String>,
}

static TOKEN_CACHE: LazyLock<Mutex<HashMap<String, TokenEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Deserialize)]
struct SessionResponse {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    expires: Option<String>,
    user: Option<UserInfo>,
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    id: Option<String>,
}

pub async fn exchange_session(
    fetch: &dyn HttpFetch,
    cookie: &str,
) -> Result<TokenEntry, Error> {
    let key = cookie_key(cookie);
    if let Some(entry) = token_lookup(&key) {
        return Ok(entry);
    }

    let mut headers = browser_headers();
    headers.push(("Accept".into(), "application/json".into()));
    headers.push(("Cookie".into(), build_session_cookie_header(cookie)));

    let resp = fetch
        .fetch(FetchRequest {
            url: SESSION_URL.into(),
            method: "GET".into(),
            headers,
            body: None,
            timeout_ms: 30_000,
        })
        .await?;

    if resp.status == 401 || resp.status == 403 {
        return Err(Error::SessionAuth("Invalid session cookie".into()));
    }
    if resp.status >= 400 {
        return Err(Error::Upstream {
            status: resp.status,
            message: format!("Session exchange failed (HTTP {})", resp.status),
        });
    }

    let set_cookie = resp.header("set-cookie");
    let refreshed = merge_refreshed_cookie(cookie, set_cookie.as_deref());
    let data: SessionResponse =
        serde_json::from_slice(&resp.body).unwrap_or(SessionResponse {
            access_token: None,
            expires: None,
            user: None,
        });
    let access_token = data
        .access_token
        .ok_or_else(|| Error::SessionAuth("missing accessToken".into()))?;

    let expires_at = token_expires_at(data.expires.as_deref());

    let entry = TokenEntry {
        access_token,
        account_id: data.user.and_then(|u| u.id),
        expires_at,
        refreshed_cookie: refreshed,
    };
    token_store(&key, entry.clone());
    Ok(entry)
}

fn token_expires_at(session_expires: Option<&str>) -> Instant {
    session_expires
        .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
        .and_then(|dt| {
            (dt.with_timezone(&chrono::Utc) - chrono::Utc::now())
                .to_std()
                .ok()
                .filter(|d| !d.is_zero())
        })
        .map(|remaining| Instant::now() + remaining)
        .unwrap_or_else(|| Instant::now() + Duration::from_millis(TOKEN_TTL_MS))
}

fn token_lookup(key: &str) -> Option<TokenEntry> {
    let cache = TOKEN_CACHE.lock().ok()?;
    let entry = cache.get(key)?;
    if entry.expires_at > Instant::now() {
        Some(entry.clone())
    } else {
        None
    }
}

fn token_store(key: &str, entry: TokenEntry) {
    if let Ok(mut cache) = TOKEN_CACHE.lock() {
        if cache.len() >= 200 {
            cache.clear();
        }
        cache.insert(key.to_string(), entry);
    }
}

pub fn invalidate_token_cache(cookie: &str) {
    if let Ok(mut cache) = TOKEN_CACHE.lock() {
        cache.remove(&cookie_key(cookie));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tls::fetch::{FetchResponse, MockFetch};

    #[tokio::test]
    async fn caches_session_token() {
        let body = br#"{"accessToken":"tok123","user":{"id":"u1"}}"#.to_vec();
        let fetch = MockFetch::new(vec![FetchResponse {
            status: 200,
            headers: vec![],
            body: body.clone(),
        }]);
        let a = exchange_session(fetch.as_ref(), "abc").await.unwrap();
        let b = exchange_session(fetch.as_ref(), "abc").await.unwrap();
        assert_eq!(a.access_token, b.access_token);
        assert_eq!(fetch.call_count(), 1);
    }
}
