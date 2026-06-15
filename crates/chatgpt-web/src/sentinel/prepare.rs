use serde::Deserialize;

use crate::{
    Error,
    constants::{SENTINEL_CR_URL, SENTINEL_PREPARE_URL},
    headers::{browser_headers, oai_headers},
    sentinel::{dpl::build_prekey_config, pow::build_prepare_token},
    session::cookie::build_session_cookie_header,
    tls::fetch::{FetchRequest, HttpFetch},
};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChatRequirements {
    pub token: Option<String>,
    pub prepare_token: Option<String>,
    pub proofofwork: Option<ProofOfWork>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProofOfWork {
    pub required: Option<bool>,
    pub seed: Option<String>,
    pub difficulty: Option<String>,
}

pub struct PrepareChatInput<'a> {
    pub access_token: &'a str,
    pub account_id: Option<&'a str>,
    pub session_id: &'a str,
    pub device_id: &'a str,
    pub cookie: &'a str,
    pub dpl: &'a str,
    pub script_src: &'a str,
}

pub async fn prepare_chat_requirements(
    fetch: &dyn HttpFetch,
    input: PrepareChatInput<'_>,
) -> Result<ChatRequirements, Error> {
    let PrepareChatInput {
        access_token,
        account_id,
        session_id,
        device_id,
        cookie,
        dpl,
        script_src,
    } = input;

    let config = build_prekey_config(
        crate::constants::CHATGPT_USER_AGENT,
        dpl,
        script_src,
    );
    let prekey = build_prepare_token(config);

    let mut headers = browser_headers();
    headers.extend(oai_headers(session_id, device_id));
    headers.push(("Content-Type".into(), "application/json".into()));
    headers.push(("Authorization".into(), format!("Bearer {access_token}")));
    headers.push(("Cookie".into(), build_session_cookie_header(cookie)));
    headers.push(("Priority".into(), "u=1, i".into()));
    if let Some(id) = account_id {
        headers.push(("chatgpt-account-id".into(), id.to_string()));
    }

    let prep_resp = fetch
        .fetch(FetchRequest {
            url: SENTINEL_PREPARE_URL.into(),
            method: "POST".into(),
            headers: headers.clone(),
            body: Some(
                serde_json::json!({ "p": prekey }).to_string().into_bytes(),
            ),
            timeout_ms: 30_000,
        })
        .await?;

    if prep_resp.status == 401 || prep_resp.status == 403 {
        return Err(Error::SentinelBlocked(format!(
            "Sentinel /prepare blocked (HTTP {})",
            prep_resp.status
        )));
    }
    if prep_resp.status >= 400 {
        return Err(Error::Upstream {
            status: prep_resp.status,
            message: "Sentinel /prepare failed".into(),
        });
    }

    let prep_data: ChatRequirements =
        serde_json::from_slice(&prep_resp.body).unwrap_or_default();
    let Some(prepare_token) = prep_data.prepare_token.clone() else {
        return Ok(prep_data);
    };

    let cr_resp = fetch
        .fetch(FetchRequest {
            url: SENTINEL_CR_URL.into(),
            method: "POST".into(),
            headers,
            body: Some(
                serde_json::json!({ "p": prekey, "prepare_token": prepare_token })
                    .to_string()
                    .into_bytes(),
            ),
            timeout_ms: 30_000,
        })
        .await?;

    if cr_resp.status == 401 || cr_resp.status == 403 {
        return Err(Error::SentinelBlocked(format!(
            "Sentinel /chat-requirements blocked (HTTP {})",
            cr_resp.status
        )));
    }
    if cr_resp.status >= 400 {
        return Ok(prep_data);
    }

    let mut cr_data: ChatRequirements =
        serde_json::from_slice(&cr_resp.body).unwrap_or_default();
    cr_data.prepare_token = Some(prepare_token);
    Ok(cr_data)
}
