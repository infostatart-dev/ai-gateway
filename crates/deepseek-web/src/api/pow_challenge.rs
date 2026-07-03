use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    Error,
    constants::{POW_CHALLENGE_URL, POW_TARGET_PATH},
    headers::json_headers_for_session,
    session::file::BrowserSession,
    tls::fetch::{FetchRequest, HttpFetch},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PowChallenge {
    pub algorithm: String,
    pub challenge: String,
    pub salt: String,
    pub signature: String,
    pub difficulty: u32,
    pub expire_at: i64,
    #[serde(default)]
    pub expire_after: i64,
    pub target_path: String,
}

pub async fn create_pow_challenge(
    fetch: &dyn HttpFetch,
    access_token: &str,
) -> Result<PowChallenge, Error> {
    create_pow_challenge_with_browser(fetch, access_token, None).await
}

pub async fn create_pow_challenge_with_browser(
    fetch: &dyn HttpFetch,
    access_token: &str,
    session: Option<&BrowserSession>,
) -> Result<PowChallenge, Error> {
    let body =
        serde_json::json!({ "target_path": POW_TARGET_PATH }).to_string();
    let resp = fetch
        .fetch(FetchRequest {
            url: POW_CHALLENGE_URL.into(),
            method: "POST".into(),
            headers: json_headers_for_session(access_token, session),
            body: Some(body.into_bytes()),
            timeout_ms: 30_000,
        })
        .await?;

    if resp.status >= 400 {
        return Err(Error::Upstream {
            status: resp.status,
            message: format!("create_pow_challenge HTTP {}", resp.status),
        });
    }

    challenge_from_json(&resp.body)
}

fn challenge_from_json(body: &[u8]) -> Result<PowChallenge, Error> {
    let v: Value = serde_json::from_slice(body)?;
    let code = v.get("code").and_then(Value::as_i64);
    let node = v
        .pointer("/data/biz_data/challenge")
        .or_else(|| v.pointer("/biz_data/challenge"))
        .ok_or_else(|| {
            Error::Other(format!("No PoW challenge: code={code:?}"))
        })?;
    serde_json::from_value(node.clone()).map_err(Error::from)
}
