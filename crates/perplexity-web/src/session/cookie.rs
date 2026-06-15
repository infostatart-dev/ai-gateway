use sha2::{Digest, Sha256};

const SESSION_TOKEN_PREFIX: &str = "__Secure-next-auth.session-token";
const CF_COOKIE_NAMES: &[&str] = &["cf_clearance", "__cf_bm", "_cfuvid"];

#[must_use]
pub fn has_session_token(cookie_header: &str) -> bool {
    cookie_header.contains(SESSION_TOKEN_PREFIX)
}

#[must_use]
pub fn cookie_usable(cookie_header: &str) -> bool {
    has_session_token(cookie_header)
}

#[must_use]
pub fn build_session_cookie_header(raw_input: &str) -> String {
    normalize_cookie_blob(raw_input)
}

#[must_use]
pub fn normalize_cookie_blob(raw_input: &str) -> String {
    let mut s = raw_input.trim().to_string();
    if let Some(stripped) = s
        .strip_prefix("Cookie:")
        .or_else(|| s.strip_prefix("cookie:"))
    {
        s = stripped.trim().to_string();
    }
    if !s.contains('=') {
        return format!("{SESSION_TOKEN_PREFIX}={s}");
    }

    let mut order = Vec::new();
    let mut values = std::collections::HashMap::new();
    for pair in s.split(';') {
        let pair = pair.trim();
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        let name = name.trim().to_string();
        if !name.starts_with(SESSION_TOKEN_PREFIX)
            && !CF_COOKIE_NAMES.contains(&name.as_str())
        {
            continue;
        }
        if !values.contains_key(&name) {
            order.push(name.clone());
        }
        values.insert(name, value.trim().to_string());
    }
    order
        .into_iter()
        .filter_map(|name| values.get(&name).map(|v| format!("{name}={v}")))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Browser cookie pairs after login (OmniRoute: session-token + CF helpers).
#[must_use]
pub fn format_login_cookie_pairs(pairs: &[(String, String)]) -> Option<String> {
    let mut parts = Vec::new();
    let mut has_session = false;
    for (name, value) in pairs {
        if name.starts_with(SESSION_TOKEN_PREFIX) {
            parts.push(format!("{name}={value}"));
            has_session = true;
        } else if CF_COOKIE_NAMES.contains(&name.as_str()) {
            parts.push(format!("{name}={value}"));
        }
    }
    if has_session {
        Some(normalize_cookie_blob(&parts.join("; ")))
    } else {
        None
    }
}

#[must_use]
pub fn cookie_key(cookie: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(cookie.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()[..16]
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_token_gets_prefix() {
        assert_eq!(
            build_session_cookie_header("eyJhbGc"),
            "__Secure-next-auth.session-token=eyJhbGc"
        );
    }

    #[test]
    fn cf_only_is_not_usable() {
        assert!(!cookie_usable("cf_clearance=abc"));
    }

    #[test]
    fn format_login_pairs_includes_cf_cookies() {
        let pairs = vec![
            ("__Secure-next-auth.session-token".into(), "eyJ".into()),
            ("cf_clearance".into(), "abc".into()),
            ("unrelated".into(), "skip".into()),
        ];
        let out = format_login_cookie_pairs(&pairs).unwrap();
        assert!(out.contains("__Secure-next-auth.session-token=eyJ"));
        assert!(out.contains("cf_clearance=abc"));
        assert!(!out.contains("unrelated"));
    }

    #[test]
    fn chunked_session_token_from_browser() {
        let pairs = vec![
            ("__Secure-next-auth.session-token.0".into(), "aaa".into()),
            ("__Secure-next-auth.session-token.1".into(), "bbb".into()),
            ("cf_clearance".into(), "x".into()),
        ];
        let out = format_login_cookie_pairs(&pairs).unwrap();
        assert!(out.contains("__Secure-next-auth.session-token.0=aaa"));
        assert!(out.contains("cf_clearance=x"));
    }
}
