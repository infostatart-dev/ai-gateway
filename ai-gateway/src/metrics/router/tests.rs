use compact_str::CompactString;
use http::StatusCode;

use crate::{
    metrics::router::{FailoverEvent, base_router_kv},
    router::retry_after::{
        FailoverClass, quota_metric_from_status, quota_metric_label,
    },
    types::{
        extensions::RouterRuntimeLabels, provider::InferenceProvider,
        router::RouterId,
    },
};

#[test]
fn base_router_kv_contains_expected_keys() {
    let rtl = RouterRuntimeLabels {
        router_id: RouterId::Named(CompactString::new("test-router")),
        endpoint_type: "chat".to_string(),
        strategy: "provider-weighted",
    };
    let kv = base_router_kv(&rtl);
    assert_eq!(kv.len(), 3);
    let keys: Vec<_> = kv.iter().map(|k| k.key.as_str()).collect();
    assert!(keys.contains(&"router_id"));
    assert!(keys.contains(&"endpoint_type"));
    assert!(keys.contains(&"strategy"));
}

#[test]
fn quota_metric_labels_cover_limit_dimensions() {
    assert_eq!(
        quota_metric_label(
            StatusCode::TOO_MANY_REQUESTS,
            FailoverClass::QuotaExhausted
        ),
        "rpd"
    );
    assert_eq!(
        quota_metric_label(
            StatusCode::PAYLOAD_TOO_LARGE,
            FailoverClass::Transient
        ),
        "tpm"
    );
    assert_eq!(
        quota_metric_label(
            StatusCode::SERVICE_UNAVAILABLE,
            FailoverClass::Overload
        ),
        "overload"
    );
    assert_eq!(
        quota_metric_from_status(StatusCode::PAYLOAD_TOO_LARGE),
        "tpm"
    );
}

#[test]
fn failover_event_carries_credential_and_quota_metric() {
    let router_id = RouterId::Named(CompactString::new("autodefault"));
    let provider = InferenceProvider::GoogleGemini;
    let event = FailoverEvent {
        router_id: &router_id,
        endpoint_type: "chat",
        strategy: "budget-aware-capability-after",
        from_provider: &provider,
        to_provider: Some(&InferenceProvider::Named("groq".into())),
        reason: "429",
        credential: "gemini-free",
        quota_metric: "rpd",
    };
    assert_eq!(event.credential, "gemini-free");
    assert_eq!(event.quota_metric, "rpd");
}
