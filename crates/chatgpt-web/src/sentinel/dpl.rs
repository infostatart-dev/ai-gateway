use std::sync::{LazyLock, Mutex};
use std::time::Instant;

use rand::Rng;

use crate::constants::{CHATGPT_BASE, DPL_TTL_MS, OAI_CLIENT_VERSION};
use crate::headers::browser_headers;
use crate::session::cookie::build_session_cookie_header;
use crate::tls::fetch::{FetchRequest, HttpFetch};
use crate::Error;

#[derive(Debug, Clone)]
pub struct DplInfo {
    pub dpl: String,
    pub script_src: String,
    expires_at: Instant,
}

static DPL_CACHE: LazyLock<Mutex<Option<DplInfo>>> = LazyLock::new(|| Mutex::new(None));

pub async fn fetch_dpl(fetch: &dyn HttpFetch, cookie: &str) -> Result<(String, String), Error> {
    if let Ok(guard) = DPL_CACHE.lock() {
        if let Some(ref info) = *guard {
            if info.expires_at > Instant::now() {
                return Ok((info.dpl.clone(), info.script_src.clone()));
            }
        }
    }

    let mut headers = browser_headers();
    headers.push((
        "Accept".into(),
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".into(),
    ));
    headers.push(("Cookie".into(), build_session_cookie_header(cookie)));

    let resp = fetch
        .fetch(FetchRequest {
            url: format!("{CHATGPT_BASE}/"),
            method: "GET".into(),
            headers,
            body: None,
            timeout_ms: 20_000,
        })
        .await?;

    let html = String::from_utf8_lossy(&resp.body);
    let dpl = regex::Regex::new(r#"data-build="([^"]+)""#)
        .ok()
        .and_then(|re| re.captures(&html))
        .and_then(|c| c.get(1).map(|m| format!("dpl={}", m.as_str())))
        .unwrap_or_else(|| format!("dpl={}", OAI_CLIENT_VERSION.trim_start_matches("prod-")));

    let script_src = regex::Regex::new(r#"<script[^>]+src="(https?://[^"]*\.js[^"]*)""#)
        .ok()
        .and_then(|re| re.captures(&html))
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .unwrap_or_else(|| {
            format!(
                "{CHATGPT_BASE}/_next/static/chunks/webpack-{}.js",
                random_hex(16)
            )
        });

    if let Ok(mut guard) = DPL_CACHE.lock() {
        *guard = Some(DplInfo {
            dpl: dpl.clone(),
            script_src: script_src.clone(),
            expires_at: Instant::now() + std::time::Duration::from_millis(DPL_TTL_MS),
        });
    }
    Ok((dpl, script_src))
}

#[cfg(test)]
pub fn clear_dpl_cache() {
    if let Ok(mut guard) = DPL_CACHE.lock() {
        *guard = None;
    }
}

pub fn build_prekey_config(user_agent: &str, dpl: &str, script_src: &str) -> Vec<serde_json::Value> {
    let screen_sizes = [3000, 4000, 3120, 4160];
    let cores = [8, 16, 24, 32];
    let mut rng = rand::rng();
    let perf_now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64
        % 10_000.0;
    let epoch_offset = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64)
        - perf_now;

    vec![
        serde_json::json!(screen_sizes[rng.random_range(0..screen_sizes.len())]),
        serde_json::json!(chrono::Utc::now().to_rfc3339()),
        serde_json::json!(4294705152_i64),
        serde_json::json!(0),
        serde_json::json!(user_agent),
        serde_json::json!(script_src),
        serde_json::json!(dpl),
        serde_json::json!("en-US"),
        serde_json::json!("en-US,en"),
        serde_json::json!(0),
        serde_json::json!("webdriver−false"),
        serde_json::json!("_reactListeningkfj3eavmks"),
        serde_json::json!("webpackChunk_N_E"),
        serde_json::json!(perf_now),
        serde_json::json!(uuid::Uuid::new_v4().to_string()),
        serde_json::json!(""),
        serde_json::json!(cores[rng.random_range(0..cores.len())]),
        serde_json::json!(epoch_offset),
    ]
}

fn random_hex(n: usize) -> String {
    let mut rng = rand::rng();
    (0..n)
        .map(|_| format!("{:x}", rng.random_range(0u8..16)))
        .collect()
}

pub fn fallback_dpl() -> (String, String) {
    (
        format!("dpl={}", OAI_CLIENT_VERSION.trim_start_matches("prod-")),
        format!(
            "{CHATGPT_BASE}/_next/static/chunks/webpack-{}.js",
            random_hex(16)
        ),
    )
}
