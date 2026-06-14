use sha2::{Digest, Sha256};

const SESSION_TOKEN_PREFIX: &str = "__Secure-next-auth.session-token";

/// Cloudflare cookies that must travel with the session token (OmniRoute DevTools flow).
const CF_COOKIE_NAMES: &[&str] = &["cf_clearance", "__cf_bm", "_cfuvid"];

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

pub fn build_session_cookie_header(raw_input: &str) -> String {
    normalize_cookie_blob(raw_input)
}

/// Dedupe cookie pairs and keep only session-token + Cloudflare helpers (OmniRoute DevTools set).
#[must_use]
pub fn normalize_cookie_blob(raw_input: &str) -> String {
    let mut s = raw_input.trim().to_string();
    if let Some(stripped) = s.strip_prefix("Cookie:").or_else(|| s.strip_prefix("cookie:")) {
        s = stripped.trim().to_string();
    }
    if !s.contains('=') {
        return format!("{SESSION_TOKEN_PREFIX}={s}");
    }

    let mut order: Vec<String> = Vec::new();
    let mut values: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for pair in s.split(';') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        let name = name.trim().to_string();
        if !name.starts_with(SESSION_TOKEN_PREFIX) && !CF_COOKIE_NAMES.contains(&name.as_str()) {
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

pub fn merge_refreshed_cookie(existing: &str, set_cookie: Option<&str>) -> Option<String> {
    let set_cookie = set_cookie?;
    let mut refreshed = Vec::new();
    for part in set_cookie.split(',') {
        let part = part.trim();
        let (name, value) = part.split_once('=')?;
        let name = name.trim();
        if name.starts_with("__Secure-next-auth.session-token") {
            refreshed.push((name.to_string(), value.split(';').next()?.trim().to_string()));
        }
    }
    if refreshed.is_empty() {
        return None;
    }
    let mut kept: Vec<String> = existing
        .split(';')
        .filter_map(|pair| {
            let pair = pair.trim();
            let (name, _) = pair.split_once('=')?;
            if name.starts_with("__Secure-next-auth.session-token") {
                None
            } else {
                Some(pair.to_string())
            }
        })
        .collect();
    for (name, value) in refreshed {
        kept.push(format!("{name}={value}"));
    }
    Some(normalize_cookie_blob(&kept.join("; ")))
}

/// Build a Cookie header from browser cookie pairs after login (OmniRoute-style).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_value_gets_prefix() {
        assert_eq!(
            build_session_cookie_header("eyJhbGc"),
            "__Secure-next-auth.session-token=eyJhbGc"
        );
    }

    #[test]
    fn unchunked_passthrough() {
        let c = "__Secure-next-auth.session-token=abc";
        assert_eq!(build_session_cookie_header(c), c);
    }

    #[test]
    fn devtools_prefix_stripped() {
        let c = "Cookie: __Secure-next-auth.session-token=abc; cf_clearance=x";
        assert_eq!(
            build_session_cookie_header(c),
            "__Secure-next-auth.session-token=abc; cf_clearance=x"
        );
    }

    #[test]
    fn chunked_passthrough() {
        let c = "__Secure-next-auth.session-token.0=a; __Secure-next-auth.session-token.1=b";
        assert_eq!(build_session_cookie_header(c), c);
    }

    #[test]
    fn format_login_pairs_includes_cf_cookies() {
        let pairs = vec![
            (
                "__Secure-next-auth.session-token".into(),
                "eyJ".into(),
            ),
            ("cf_clearance".into(), "abc".into()),
            ("unrelated".into(), "skip".into()),
        ];
        let out = format_login_cookie_pairs(&pairs).unwrap();
        assert!(out.contains("__Secure-next-auth.session-token=eyJ"));
        assert!(out.contains("cf_clearance=abc"));
        assert!(!out.contains("unrelated"));
    }
}
