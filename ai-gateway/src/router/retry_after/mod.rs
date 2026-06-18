mod body;
mod classify;
mod constants;
mod duration;
mod header;
mod text;

mod abuse;

mod quota_scope;

use std::time::Duration;

use abuse::{looks_like_abuse_block, looks_like_unsupported_model};
pub use body::extract_retry_after_from_body;
use bytes::Bytes;
pub use classify::FailureKind;
use classify::classify_429;
pub use header::extract_retry_after_from_headers;
use http::{HeaderMap, StatusCode};
use http_body_util::BodyExt;
use quota_scope::classify_exhaustion_scope;
pub use quota_scope::{
    ExhaustionScope, phantom_model_cooldown_applies, slot_cooldown_also_applies,
};

use crate::{
    config::{
        provider_limits::ProviderQuotaProfile,
        router_cooldown::RouterCooldownConfig,
    },
    types::response::Response,
};

/// How a failed candidate should influence same-provider sibling failover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailoverClass {
    /// Transient (RPM rate-limit, schema, generic): try the next sibling.
    Transient,
    /// Daily/quota exhaustion: skip remaining free siblings, go paid/next.
    QuotaExhausted,
    /// Upstream overload (`502`/`503`): skip remaining free siblings.
    Overload,
}

/// Best-effort quota-dimension label inferred from the HTTP status alone, for
/// callers that do not classify the response body (e.g. provider-level
/// failover).
#[must_use]
pub fn quota_metric_from_status(status: StatusCode) -> &'static str {
    let class = match status {
        StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE => {
            FailoverClass::Overload
        }
        _ => FailoverClass::Transient,
    };
    quota_metric_label(status, class)
}

/// The metric label describing which quota dimension a failure hit.
#[must_use]
pub fn quota_metric_label(
    status: StatusCode,
    class: FailoverClass,
) -> &'static str {
    match class {
        FailoverClass::Overload => "overload",
        FailoverClass::QuotaExhausted => "rpd",
        FailoverClass::Transient if status == StatusCode::TOO_MANY_REQUESTS => {
            "rpm"
        }
        FailoverClass::Transient if status == StatusCode::PAYLOAD_TOO_LARGE => {
            "tpm"
        }
        FailoverClass::Transient => "other",
    }
}

#[must_use]
pub fn rate_limit_cooldown(
    headers: &HeaderMap,
    body: Option<&[u8]>,
    config: &RouterCooldownConfig,
) -> Duration {
    let header_secs = extract_retry_after_from_headers(headers);
    let failure_kind = classify_429(body);
    let base_secs = body::resolve_429_base_secs(
        body,
        header_secs,
        failure_kind,
        config.rate_limit.as_secs(),
        config.quota_exhausted.as_secs(),
    );
    Duration::from_secs(base_secs) + config.retry_after_buffer
}

async fn collect_response_body(
    response: Response,
) -> (http::response::Parts, Bytes) {
    let (parts, body) = response.into_parts();
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => Bytes::new(),
    };
    (parts, body_bytes)
}

fn abuse_block_cooldown(config: &RouterCooldownConfig) -> Duration {
    config.abuse_block + config.retry_after_buffer
}

fn apply_model_cooldown_override(
    status: StatusCode,
    body: Option<&[u8]>,
    scope: ExhaustionScope,
    profile: ProviderQuotaProfile,
    config: &RouterCooldownConfig,
    cooldown: Duration,
) -> Duration {
    if phantom_model_cooldown_applies(status, body, scope, profile) {
        config.quota_exhausted + config.retry_after_buffer
    } else {
        cooldown
    }
}

pub async fn cooldown_for_response(
    response: Response,
    config: &RouterCooldownConfig,
    profile: ProviderQuotaProfile,
) -> (Response, Duration, ExhaustionScope) {
    let (response, cooldown, _class, scope, _) =
        classify_and_cooldown(response, config, profile).await;
    (response, cooldown, scope)
}

/// Like [`cooldown_for_response`] but also returns the [`FailoverClass`] so the
/// router can decide whether to skip remaining same-provider free siblings.
#[allow(clippy::too_many_lines)]
pub async fn classify_and_cooldown(
    response: Response,
    config: &RouterCooldownConfig,
    profile: ProviderQuotaProfile,
) -> (
    Response,
    Duration,
    FailoverClass,
    ExhaustionScope,
    Option<Duration>,
) {
    let status = response.status();
    if status == StatusCode::TOO_MANY_REQUESTS {
        if extract_retry_after_from_headers(response.headers()).is_some() {
            let cooldown =
                rate_limit_cooldown(response.headers(), None, config);
            let class = FailoverClass::Transient;
            let scope = classify_exhaustion_scope(status, None, class, profile);
            return (response, cooldown, class, scope, None);
        }
        let (parts, body_bytes) = collect_response_body(response).await;
        let cooldown = rate_limit_cooldown(
            &parts.headers,
            Some(body_bytes.as_ref()),
            config,
        );
        let class = match classify_429(Some(body_bytes.as_ref())) {
            FailureKind::QuotaExhausted => FailoverClass::QuotaExhausted,
            FailureKind::RateLimit => FailoverClass::Transient,
        };
        let scope = classify_exhaustion_scope(
            status,
            Some(body_bytes.as_ref()),
            class,
            profile,
        );
        let response = Response::from_parts(
            parts,
            axum_core::body::Body::from(body_bytes),
        );
        return (response, cooldown, class, scope, None);
    }

    if matches!(status, StatusCode::BAD_REQUEST) {
        let (parts, body_bytes) = collect_response_body(response).await;
        let cooldown =
            if looks_like_unsupported_model(Some(body_bytes.as_ref())) {
                tracing::warn!(
                    cooldown_kind = "auth-error",
                    "upstream 400 classified as unsupported model"
                );
                config.auth_error + config.retry_after_buffer
            } else {
                config.provider_error + config.retry_after_buffer
            };
        let class = FailoverClass::Transient;
        let scope = classify_exhaustion_scope(
            status,
            Some(body_bytes.as_ref()),
            class,
            profile,
        );
        let cooldown = apply_model_cooldown_override(
            status,
            Some(body_bytes.as_ref()),
            scope,
            profile,
            config,
            cooldown,
        );
        let response = Response::from_parts(
            parts,
            axum_core::body::Body::from(body_bytes),
        );
        return (response, cooldown, class, scope, None);
    }

    if matches!(
        status,
        StatusCode::UNAUTHORIZED
            | StatusCode::FORBIDDEN
            | StatusCode::PAYMENT_REQUIRED
    ) {
        let (parts, body_bytes) = collect_response_body(response).await;
        let cooldown = if looks_like_abuse_block(Some(body_bytes.as_ref())) {
            tracing::warn!(
                cooldown_kind = "abuse-block",
                "auth response classified as abuse block"
            );
            abuse_block_cooldown(config)
        } else {
            config.auth_error
        };
        let class = FailoverClass::Transient;
        let scope = classify_exhaustion_scope(
            status,
            Some(body_bytes.as_ref()),
            class,
            profile,
        );
        let response = Response::from_parts(
            parts,
            axum_core::body::Body::from(body_bytes),
        );
        return (response, cooldown, class, scope, None);
    }

    if matches!(
        status,
        StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE
    ) {
        let (parts, body_bytes) = collect_response_body(response).await;
        let cooldown = if looks_like_abuse_block(Some(body_bytes.as_ref())) {
            tracing::warn!(
                cooldown_kind = "abuse-block",
                "upstream response classified as abuse block"
            );
            abuse_block_cooldown(config)
        } else {
            config.provider_error + config.retry_after_buffer
        };
        let class = FailoverClass::Overload;
        let scope = classify_exhaustion_scope(
            status,
            Some(body_bytes.as_ref()),
            class,
            profile,
        );
        let slot_cooldown = slot_cooldown_also_applies(
            status,
            Some(body_bytes.as_ref()),
            profile,
        )
        .then(|| config.provider_error + config.retry_after_buffer);
        let response = Response::from_parts(
            parts,
            axum_core::body::Body::from(body_bytes),
        );
        return (response, cooldown, class, scope, slot_cooldown);
    }

    if status == StatusCode::NOT_FOUND {
        let class = FailoverClass::Transient;
        let scope = classify_exhaustion_scope(status, None, class, profile);
        let cooldown = apply_model_cooldown_override(
            status,
            None,
            scope,
            profile,
            config,
            config.provider_error + config.retry_after_buffer,
        );
        return (response, cooldown, class, scope, None);
    }

    let class = FailoverClass::Transient;
    let scope = classify_exhaustion_scope(status, None, class, profile);
    (response, config.provider_error, class, scope, None)
}

#[cfg(test)]
mod tests {
    use axum_core::body::Body;
    use http::StatusCode;

    use super::*;
    use crate::{
        config::{
            provider_limits::ProviderLimitCatalog,
            router_cooldown::RouterCooldownConfig,
        },
        types::provider::InferenceProvider,
    };

    #[tokio::test]
    async fn cooldown_uses_gemini_retry_delay_from_body() {
        let body = br#"{"error":{"details":[{"retryDelay":"15.002899939s"}]}}"#;
        let response = http::Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .body(Body::from(body.as_slice()))
            .unwrap();
        let config = RouterCooldownConfig::default();
        let (_, cooldown) = {
            let (response, cooldown, _) = cooldown_for_response(
                response,
                &config,
                ProviderQuotaProfile::PerModel,
            )
            .await;
            (response, cooldown)
        };
        assert_eq!(
            cooldown,
            Duration::from_secs(16) + config.retry_after_buffer
        );
    }

    #[test]
    fn quota_exhausted_uses_long_fallback_without_reset_hint() {
        let body = br#"{"error":{"message":"You exceeded your daily limit."}}"#;
        let config = RouterCooldownConfig::default();
        let cooldown = rate_limit_cooldown(
            &HeaderMap::new(),
            Some(body.as_ref()),
            &config,
        );
        assert_eq!(
            cooldown,
            config.quota_exhausted + config.retry_after_buffer
        );
    }

    #[test]
    fn cloudflare_quota_exhausted_uses_provider_override() {
        let catalog = ProviderLimitCatalog::default();
        let config = catalog
            .cooldown_for(&InferenceProvider::Named("cloudflare".into()));
        let body =
            br#"{"error":{"message":"daily free allocation exhausted"}}"#;
        let cooldown = rate_limit_cooldown(
            &HeaderMap::new(),
            Some(body.as_ref()),
            &config,
        );
        assert_eq!(
            cooldown,
            Duration::from_secs(24 * 3600) + config.retry_after_buffer
        );
    }

    #[tokio::test]
    async fn abuse_502_uses_abuse_block_cooldown_for_chatgpt_web() {
        let catalog = ProviderLimitCatalog::default();
        let config = catalog
            .cooldown_for(&InferenceProvider::Named("chatgpt-web".into()));
        let body = br#"{"error":{"message":"Our systems have detected unusual activity"}}"#;
        let response = http::Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Body::from(body.as_slice()))
            .unwrap();
        let (_, cooldown) = {
            let (response, cooldown, _) = cooldown_for_response(
                response,
                &config,
                ProviderQuotaProfile::PerSlot,
            )
            .await;
            (response, cooldown)
        };
        assert_eq!(cooldown, config.abuse_block + config.retry_after_buffer);
        assert_eq!(cooldown, Duration::from_secs(4 * 3600 + 1));
    }

    #[tokio::test]
    async fn generic_502_uses_provider_error_cooldown() {
        let catalog = ProviderLimitCatalog::default();
        let config = catalog
            .cooldown_for(&InferenceProvider::Named("chatgpt-web".into()));
        let body = br#"{"error":{"message":"upstream connection reset"}}"#;
        let response = http::Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Body::from(body.as_slice()))
            .unwrap();
        let (_, cooldown) = {
            let (response, cooldown, _) = cooldown_for_response(
                response,
                &config,
                ProviderQuotaProfile::PerSlot,
            )
            .await;
            (response, cooldown)
        };
        assert_eq!(cooldown, config.provider_error + config.retry_after_buffer);
        assert_eq!(cooldown, Duration::from_secs(60 + 1));
    }

    #[tokio::test]
    async fn per_model_404_uses_long_model_cooldown() {
        let config = RouterCooldownConfig::default();
        let response = http::Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("model not found"))
            .unwrap();
        let (_, cooldown, scope) = cooldown_for_response(
            response,
            &config,
            ProviderQuotaProfile::PerModel,
        )
        .await;
        assert_eq!(scope, ExhaustionScope::Model);
        assert_eq!(
            cooldown,
            config.quota_exhausted + config.retry_after_buffer
        );
    }
}
