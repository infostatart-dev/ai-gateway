use crate::constants::{
    CHATGPT_BASE, CHATGPT_USER_AGENT, OAI_CLIENT_BUILD_NUMBER, OAI_CLIENT_VERSION,
};

pub fn browser_headers() -> Vec<(String, String)> {
    vec![
        ("Accept".into(), "*/*".into()),
        ("Accept-Language".into(), "en-US,en;q=0.9".into()),
        ("Cache-Control".into(), "no-cache".into()),
        ("Origin".into(), CHATGPT_BASE.into()),
        ("Pragma".into(), "no-cache".into()),
        ("Referer".into(), format!("{CHATGPT_BASE}/")),
        ("Sec-Fetch-Dest".into(), "empty".into()),
        ("Sec-Fetch-Mode".into(), "cors".into()),
        ("Sec-Fetch-Site".into(), "same-origin".into()),
        ("User-Agent".into(), CHATGPT_USER_AGENT.into()),
    ]
}

pub fn oai_headers(session_id: &str, device_id: &str) -> Vec<(String, String)> {
    vec![
        ("OAI-Language".into(), "en-US".into()),
        ("OAI-Device-Id".into(), device_id.into()),
        ("OAI-Client-Version".into(), OAI_CLIENT_VERSION.into()),
        ("OAI-Client-Build-Number".into(), OAI_CLIENT_BUILD_NUMBER.into()),
        ("OAI-Session-Id".into(), session_id.into()),
    ]
}
