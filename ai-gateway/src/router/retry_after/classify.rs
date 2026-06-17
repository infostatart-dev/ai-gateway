use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    RateLimit,
    QuotaExhausted,
}

#[must_use]
pub fn classify_429(body: Option<&[u8]>) -> FailureKind {
    if looks_like_quota_exhausted(body) {
        return FailureKind::QuotaExhausted;
    }
    FailureKind::RateLimit
}

#[must_use]
pub fn looks_like_quota_exhausted(body: Option<&[u8]>) -> bool {
    let text = body_to_text(body);
    if text.is_empty() {
        return false;
    }
    quota_patterns().iter().any(|pat| pat.is_match(&text))
}

fn body_to_text(body: Option<&[u8]>) -> String {
    let Some(bytes) = body else {
        return String::new();
    };
    if let Ok(value) = serde_json::from_slice::<Value>(bytes) {
        return value.to_string();
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn quota_patterns() -> &'static [regex::Regex] {
    use std::sync::OnceLock;
    static PATTERNS: OnceLock<Vec<regex::Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            r"(?i)daily.*limit",
            r"(?i)daily.*quota",
            r"(?i)daily.*alloc",
            r"(?i)per.?day.*limit",
            r"(?i)monthly.*limit",
            r"(?i)monthly.*quota",
            r"(?i)per.?month.*limit",
            r"(?i)quota.*exceed",
            r"(?i)exceed.*quota",
            r"(?i)insufficient.*quota",
            r"(?i)billing.*cap",
            r"(?i)credit.*exhaust",
            r"(?i)out of credits",
            r"(?i)hard.?limit",
            r"(?i)plan.*limit",
            r"(?i)resource.*exhaust",
            r"(?i)check.*quota",
            r"(?i)individual quota reached",
            r"(?i)enable overages",
        ]
        .into_iter()
        .map(|p| regex::Regex::new(p).expect("valid quota regex"))
        .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_non_quota_429_as_rate_limit() {
        assert_eq!(
            classify_429(Some(b"Too many requests, please slow down.")),
            FailureKind::RateLimit
        );
    }

    #[test]
    fn classify_daily_limit_as_quota_exhausted() {
        assert_eq!(
            classify_429(Some(b"You exceeded your daily limit.")),
            FailureKind::QuotaExhausted
        );
    }

    #[test]
    fn classify_antigravity_quota_without_overmatching_rpm() {
        assert_eq!(
            classify_429(Some(b"Request quota reached, retry in 60s.")),
            FailureKind::RateLimit
        );
        let body = b"Individual quota reached. Contact your administrator to enable overages. Resets in 164h27m24s.";
        assert!(looks_like_quota_exhausted(Some(body)));
        assert_eq!(classify_429(Some(body)), FailureKind::QuotaExhausted);
    }

    #[test]
    fn looks_like_quota_exhausted_rejects_plain_rate_limit() {
        assert!(!looks_like_quota_exhausted(Some(
            b"rate limit, please retry in 60s"
        )));
    }
}
