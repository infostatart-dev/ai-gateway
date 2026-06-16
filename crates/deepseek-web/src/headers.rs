use crate::constants::{
    APP_VERSION, CLIENT_LOCALE, CLIENT_PLATFORM, CLIENT_VERSION,
    DEEPSEEK_WEB_BASE, USER_AGENT,
};

pub fn fake_headers() -> Vec<(String, String)> {
    vec![
        ("Accept".into(), "*/*".into()),
        ("Accept-Encoding".into(), "gzip, deflate, br, zstd".into()),
        ("Accept-Language".into(), "en-US,en;q=0.9".into()),
        ("Origin".into(), DEEPSEEK_WEB_BASE.into()),
        ("Referer".into(), format!("{DEEPSEEK_WEB_BASE}/")),
        ("User-Agent".into(), USER_AGENT.into()),
        ("X-App-Version".into(), APP_VERSION.into()),
        ("X-Client-Locale".into(), CLIENT_LOCALE.into()),
        ("X-Client-Platform".into(), CLIENT_PLATFORM.into()),
        ("X-Client-Version".into(), CLIENT_VERSION.into()),
    ]
}

pub fn auth_headers(access_token: &str) -> Vec<(String, String)> {
    let mut h = fake_headers();
    h.push(("Authorization".into(), format!("Bearer {access_token}")));
    h
}

pub fn json_headers(access_token: &str) -> Vec<(String, String)> {
    let mut h = auth_headers(access_token);
    h.push(("Content-Type".into(), "application/json".into()));
    h
}
