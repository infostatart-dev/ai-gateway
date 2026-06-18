use http::StatusCode;

use super::FailoverClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExhaustionScope {
    Model,
    Slot,
    Project,
}

#[must_use]
pub fn looks_like_project_billing_cap(body: Option<&[u8]>) -> bool {
    let text = body_to_text(body);
    if text.is_empty() {
        return false;
    }
    project_patterns().iter().any(|pat| pat.is_match(&text))
}

#[must_use]
pub fn classify_exhaustion_scope(
    status: StatusCode,
    body: Option<&[u8]>,
    class: FailoverClass,
) -> ExhaustionScope {
    if matches!(status, StatusCode::PAYMENT_REQUIRED) {
        return ExhaustionScope::Project;
    }
    if matches!(class, FailoverClass::QuotaExhausted)
        && looks_like_project_billing_cap(body)
    {
        return ExhaustionScope::Project;
    }
    match class {
        FailoverClass::Transient if status == StatusCode::TOO_MANY_REQUESTS => {
            ExhaustionScope::Model
        }
        FailoverClass::QuotaExhausted => ExhaustionScope::Model,
        FailoverClass::Overload | FailoverClass::Transient => {
            ExhaustionScope::Slot
        }
    }
}

fn body_to_text(body: Option<&[u8]>) -> String {
    let Some(bytes) = body else {
        return String::new();
    };
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) {
        return value.to_string();
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn project_patterns() -> &'static [regex::Regex] {
    use std::sync::OnceLock;
    static PATTERNS: OnceLock<Vec<regex::Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            r"(?i)set up billing",
            r"(?i)enable billing",
            r"(?i)billing.*required",
            r"(?i)billing.*cap",
            r"(?i)spending.*cap",
            r"(?i)free tier.*exhaust",
            r"(?i)project.*quota",
            r"(?i)RESOURCE_EXHAUSTED.*billing",
        ]
        .into_iter()
        .map(|p| regex::Regex::new(p).expect("valid project quota regex"))
        .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_rpd_is_not_project_scope() {
        let body = br#"{"error":{"message":"You exceeded your daily limit."}}"#;
        let scope = classify_exhaustion_scope(
            StatusCode::TOO_MANY_REQUESTS,
            Some(body.as_ref()),
            FailoverClass::QuotaExhausted,
        );
        assert_eq!(scope, ExhaustionScope::Model);
        assert!(!looks_like_project_billing_cap(Some(body.as_ref())));
    }

    #[test]
    fn billing_cap_is_project_scope() {
        let body = br#"{"error":{"message":"Set up billing to continue."}}"#;
        assert!(looks_like_project_billing_cap(Some(body.as_ref())));
        let scope = classify_exhaustion_scope(
            StatusCode::TOO_MANY_REQUESTS,
            Some(body.as_ref()),
            FailoverClass::QuotaExhausted,
        );
        assert_eq!(scope, ExhaustionScope::Project);
    }

    #[test]
    fn rpm_429_is_model_scope() {
        let scope = classify_exhaustion_scope(
            StatusCode::TOO_MANY_REQUESTS,
            Some(b"rate limit"),
            FailoverClass::Transient,
        );
        assert_eq!(scope, ExhaustionScope::Model);
    }
}
