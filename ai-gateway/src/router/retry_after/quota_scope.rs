use http::StatusCode;

use super::FailoverClass;
use crate::config::provider_limits::ProviderQuotaProfile;

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
    profile: ProviderQuotaProfile,
) -> ExhaustionScope {
    if matches!(status, StatusCode::PAYMENT_REQUIRED) {
        if profile == ProviderQuotaProfile::PerModel
            && super::abuse::looks_like_unpaid_route(body)
        {
            return ExhaustionScope::Model;
        }
        return ExhaustionScope::Project;
    }
    if matches!(class, FailoverClass::QuotaExhausted)
        && looks_like_project_billing_cap(body)
    {
        return ExhaustionScope::Project;
    }

    if profile == ProviderQuotaProfile::PerModel {
        if matches!(status, StatusCode::NOT_FOUND) {
            return ExhaustionScope::Model;
        }
        if matches!(status, StatusCode::BAD_REQUEST)
            && super::abuse::looks_like_unsupported_model(body)
        {
            return ExhaustionScope::Model;
        }
        if matches!(status, StatusCode::SERVICE_UNAVAILABLE)
            && super::abuse::looks_like_high_demand(body)
        {
            return ExhaustionScope::Model;
        }
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

#[must_use]
pub fn phantom_model_cooldown_applies(
    status: StatusCode,
    body: Option<&[u8]>,
    scope: ExhaustionScope,
    profile: ProviderQuotaProfile,
) -> bool {
    profile == ProviderQuotaProfile::PerModel
        && scope == ExhaustionScope::Model
        && (status == StatusCode::NOT_FOUND
            || (status == StatusCode::BAD_REQUEST
                && super::abuse::looks_like_unsupported_model(body)))
}

#[must_use]
pub fn slot_cooldown_also_applies(
    status: StatusCode,
    body: Option<&[u8]>,
    profile: ProviderQuotaProfile,
) -> bool {
    profile == ProviderQuotaProfile::PerModel
        && status == StatusCode::SERVICE_UNAVAILABLE
        && super::abuse::looks_like_high_demand(body)
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
    use crate::config::provider_limits::ProviderQuotaProfile;

    #[test]
    fn model_rpd_is_not_project_scope() {
        let body = br#"{"error":{"message":"You exceeded your daily limit."}}"#;
        let scope = classify_exhaustion_scope(
            StatusCode::TOO_MANY_REQUESTS,
            Some(body.as_ref()),
            FailoverClass::QuotaExhausted,
            ProviderQuotaProfile::PerModel,
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
            ProviderQuotaProfile::PerModel,
        );
        assert_eq!(scope, ExhaustionScope::Project);
    }

    #[test]
    fn rpm_429_is_model_scope() {
        let scope = classify_exhaustion_scope(
            StatusCode::TOO_MANY_REQUESTS,
            Some(b"rate limit"),
            FailoverClass::Transient,
            ProviderQuotaProfile::PerModel,
        );
        assert_eq!(scope, ExhaustionScope::Model);
    }

    #[test]
    fn per_model_404_is_model_scope() {
        let scope = classify_exhaustion_scope(
            StatusCode::NOT_FOUND,
            None,
            FailoverClass::Transient,
            ProviderQuotaProfile::PerModel,
        );
        assert_eq!(scope, ExhaustionScope::Model);
    }

    #[test]
    fn per_slot_404_is_slot_scope() {
        let scope = classify_exhaustion_scope(
            StatusCode::NOT_FOUND,
            None,
            FailoverClass::Transient,
            ProviderQuotaProfile::PerSlot,
        );
        assert_eq!(scope, ExhaustionScope::Slot);
    }

    #[test]
    fn per_model_503_high_demand_classifies_as_slot_for_metrics() {
        let body = b"This model is currently experiencing high demand.";
        let scope = classify_exhaustion_scope(
            StatusCode::SERVICE_UNAVAILABLE,
            Some(body.as_ref()),
            FailoverClass::Overload,
            ProviderQuotaProfile::PerModel,
        );
        // Walk continues via Model scope (design D4); slot cooldown is
        // separate.
        assert_eq!(scope, ExhaustionScope::Model);
        assert!(slot_cooldown_also_applies(
            StatusCode::SERVICE_UNAVAILABLE,
            Some(body.as_ref()),
            ProviderQuotaProfile::PerModel,
        ));
    }

    #[test]
    fn phantom_slug_triggers_long_model_cooldown() {
        assert!(phantom_model_cooldown_applies(
            StatusCode::NOT_FOUND,
            None,
            ExhaustionScope::Model,
            ProviderQuotaProfile::PerModel,
        ));
    }

    #[test]
    fn per_model_402_unpaid_is_model_scope() {
        let body =
            br#"{"error":{"message":"You have never purchased credits."}}"#;
        let scope = classify_exhaustion_scope(
            StatusCode::PAYMENT_REQUIRED,
            Some(body.as_ref()),
            FailoverClass::Transient,
            ProviderQuotaProfile::PerModel,
        );
        assert_eq!(scope, ExhaustionScope::Model);
        assert!(crate::router::retry_after::abuse::looks_like_unpaid_route(
            Some(body.as_ref())
        ));
    }

    #[test]
    fn per_model_402_billing_cap_is_project_scope() {
        let body = br#"{"error":{"message":"Set up billing to continue."}}"#;
        let scope = classify_exhaustion_scope(
            StatusCode::PAYMENT_REQUIRED,
            Some(body.as_ref()),
            FailoverClass::Transient,
            ProviderQuotaProfile::PerModel,
        );
        assert_eq!(scope, ExhaustionScope::Project);
    }

    #[test]
    fn per_slot_402_unpaid_is_project_scope() {
        let body =
            br#"{"error":{"message":"You have never purchased credits."}}"#;
        let scope = classify_exhaustion_scope(
            StatusCode::PAYMENT_REQUIRED,
            Some(body.as_ref()),
            FailoverClass::Transient,
            ProviderQuotaProfile::PerSlot,
        );
        assert_eq!(scope, ExhaustionScope::Project);
    }
}
