use crate::{
    constants::{
        APP_VERSION, CLIENT_BUNDLE_ID, CLIENT_LOCALE, CLIENT_PLATFORM,
        CLIENT_TIMEZONE_OFFSET, CLIENT_VERSION, DEEPSEEK_WEB_BASE, USER_AGENT,
    },
    session::file::BrowserSession,
};

pub fn fake_headers() -> Vec<(String, String)> {
    browser_headers(None)
}

pub fn browser_headers(
    session: Option<&BrowserSession>,
) -> Vec<(String, String)> {
    vec![
        ("Accept".into(), "*/*".into()),
        ("Accept-Encoding".into(), "gzip, deflate, br, zstd".into()),
        (
            "Accept-Language".into(),
            session_value(session, "accept-language", "en-US,en;q=0.9"),
        ),
        ("Origin".into(), DEEPSEEK_WEB_BASE.into()),
        ("Referer".into(), format!("{DEEPSEEK_WEB_BASE}/")),
        (
            "User-Agent".into(),
            session_value(session, "user-agent", USER_AGENT),
        ),
        (
            "X-App-Version".into(),
            session_value(session, "x-app-version", APP_VERSION),
        ),
        (
            "X-Client-Bundle-Id".into(),
            session_value(session, "x-client-bundle-id", CLIENT_BUNDLE_ID),
        ),
        (
            "X-Client-Locale".into(),
            session_value(session, "x-client-locale", CLIENT_LOCALE),
        ),
        (
            "X-Client-Platform".into(),
            session_value(session, "x-client-platform", CLIENT_PLATFORM),
        ),
        (
            "X-Client-Timezone-Offset".into(),
            session_value(
                session,
                "x-client-timezone-offset",
                CLIENT_TIMEZONE_OFFSET,
            ),
        ),
        (
            "X-Client-Version".into(),
            session_value(session, "x-client-version", CLIENT_VERSION),
        ),
    ]
}

pub fn auth_headers(access_token: &str) -> Vec<(String, String)> {
    auth_headers_for_session(access_token, None)
}

pub fn auth_headers_for_session(
    access_token: &str,
    session: Option<&BrowserSession>,
) -> Vec<(String, String)> {
    let mut h = browser_headers(session);
    h.push(("Authorization".into(), format!("Bearer {access_token}")));
    if let Some(cookie) = session.and_then(|s| s.cookie.as_deref())
        && !cookie.trim().is_empty()
    {
        h.push(("Cookie".into(), cookie.to_string()));
    }
    h
}

pub fn json_headers(access_token: &str) -> Vec<(String, String)> {
    json_headers_for_session(access_token, None)
}

pub fn json_headers_for_session(
    access_token: &str,
    session: Option<&BrowserSession>,
) -> Vec<(String, String)> {
    let mut h = auth_headers_for_session(access_token, session);
    h.push(("Content-Type".into(), "application/json".into()));
    h
}

fn session_value(
    session: Option<&BrowserSession>,
    name: &str,
    default: &str,
) -> String {
    session
        .and_then(|s| s.header(name))
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(default)
        .to_string()
}
