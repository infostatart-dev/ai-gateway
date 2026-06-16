use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use crate::{
    headers::{browser_headers, oai_headers},
    session::cookie::{build_session_cookie_header, cookie_key},
    tls::fetch::{FetchRequest, HttpFetch},
};

const WARMUP_URLS: &[&str] = &[
    "https://chatgpt.com/backend-api/me",
    "https://chatgpt.com/backend-api/conversations?offset=0&limit=28&order=updated",
    "https://chatgpt.com/backend-api/models?history_and_training_disabled=false",
];

const WARMUP_TTL: Duration = Duration::from_secs(60);
const WARMUP_CACHE_MAX: usize = 200;

struct WarmupCacheEntry {
    expires_at: Instant,
    inserted_at: Instant,
}

static WARMUP_CACHE: LazyLock<Mutex<HashMap<String, WarmupCacheEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn warmup_cache_key(cookie: &str, access_token: &str) -> String {
    let suffix = access_token_suffix(access_token);
    format!("{}:{}", cookie_key(cookie), suffix)
}

fn access_token_suffix(access_token: &str) -> String {
    let len = access_token.len();
    if len <= 8 {
        access_token.to_string()
    } else {
        access_token[len - 8..].to_string()
    }
}

fn warmup_cache_hit(key: &str) -> bool {
    let Ok(cache) = WARMUP_CACHE.lock() else {
        return false;
    };
    let Some(entry) = cache.get(key) else {
        return false;
    };
    entry.expires_at > Instant::now()
}

fn warmup_cache_store(key: String) {
    if let Ok(mut cache) = WARMUP_CACHE.lock() {
        while cache.len() >= WARMUP_CACHE_MAX {
            let oldest = cache
                .iter()
                .min_by_key(|(_, e)| e.inserted_at)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                cache.remove(&k);
            } else {
                break;
            }
        }
        let now = Instant::now();
        cache.insert(
            key,
            WarmupCacheEntry {
                expires_at: now + WARMUP_TTL,
                inserted_at: now,
            },
        );
    }
}

/// Clears all warmup cache entries (tests and runtime recovery).
pub fn clear_warmup_cache() {
    if let Ok(mut cache) = WARMUP_CACHE.lock() {
        cache.clear();
    }
}

/// Drops warmup cache for one session. When `access_token` is omitted, all
/// entries for the cookie are removed.
pub fn invalidate_warmup_cache(cookie: &str, access_token: Option<&str>) {
    if let Ok(mut cache) = WARMUP_CACHE.lock() {
        if let Some(token) = access_token {
            cache.remove(&warmup_cache_key(cookie, token));
        } else {
            let prefix = format!("{}:", cookie_key(cookie));
            cache.retain(|k, _| !k.starts_with(&prefix));
        }
    }
}

#[cfg(test)]
pub fn expire_all_warmup_cache_for_test() {
    if let Ok(mut cache) = WARMUP_CACHE.lock() {
        let expired = Instant::now() - Duration::from_secs(1);
        for entry in cache.values_mut() {
            entry.expires_at = expired;
        }
    }
}

/// Best-effort browser-like warmup before sentinel (OmniRoute
/// `runSessionWarmup`).
pub async fn run_session_warmup(
    fetch: &dyn HttpFetch,
    access_token: &str,
    account_id: Option<&str>,
    session_id: &str,
    device_id: &str,
    cookie: &str,
) {
    let key = warmup_cache_key(cookie, access_token);
    if warmup_cache_hit(&key) {
        return;
    }

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

    warmup_cache_store(key);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tls::fetch::{FetchResponse, MockFetch};

    fn warmup_resp() -> FetchResponse {
        FetchResponse {
            status: 200,
            headers: vec![],
            body: br#"{}"#.to_vec(),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn cache_miss_performs_three_warmup_fetches() {
        clear_warmup_cache();
        let fetch =
            MockFetch::new(vec![warmup_resp(), warmup_resp(), warmup_resp()]);
        run_session_warmup(
            fetch.as_ref(),
            "access-token-abcdefgh",
            None,
            "sess",
            "dev",
            "cookie-a",
        )
        .await;
        assert_eq!(fetch.call_count(), 3);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn immediate_second_call_skips_warmup_fetches() {
        clear_warmup_cache();
        let fetch =
            MockFetch::new(vec![warmup_resp(), warmup_resp(), warmup_resp()]);
        let fetch_ref = fetch.as_ref();
        run_session_warmup(
            fetch_ref,
            "access-token-abcdefgh",
            None,
            "sess",
            "dev",
            "cookie-a",
        )
        .await;
        run_session_warmup(
            fetch_ref,
            "access-token-abcdefgh",
            None,
            "sess",
            "dev",
            "cookie-a",
        )
        .await;
        assert_eq!(fetch.call_count(), 3);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn warmup_runs_again_after_ttl_expires() {
        clear_warmup_cache();
        let fetch = MockFetch::new(vec![
            warmup_resp(),
            warmup_resp(),
            warmup_resp(),
            warmup_resp(),
            warmup_resp(),
            warmup_resp(),
        ]);
        let fetch_ref = fetch.as_ref();
        run_session_warmup(
            fetch_ref,
            "access-token-abcdefgh",
            None,
            "sess",
            "dev",
            "cookie-a",
        )
        .await;
        expire_all_warmup_cache_for_test();
        run_session_warmup(
            fetch_ref,
            "access-token-abcdefgh",
            None,
            "sess",
            "dev",
            "cookie-a",
        )
        .await;
        assert_eq!(fetch.call_count(), 6);
    }
}
