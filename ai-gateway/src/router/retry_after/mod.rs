mod body;
mod classify;
mod constants;
mod duration;
mod header;
mod text;

mod abuse;

use std::time::Duration;

use abuse::looks_like_abuse_block;
pub use body::extract_retry_after_from_body;
use bytes::Bytes;
use classify::classify_429;
pub use header::extract_retry_after_from_headers;
use http::{HeaderMap, StatusCode};
use http_body_util::BodyExt;

use crate::{
    config::router_cooldown::RouterCooldownConfig, types::response::Response,
};

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

pub async fn cooldown_for_response(
    response: Response,
    config: &RouterCooldownConfig,
) -> (Response, Duration) {
    let status = response.status();
    if status == StatusCode::TOO_MANY_REQUESTS {
        if extract_retry_after_from_headers(response.headers()).is_some() {
            let cooldown =
                rate_limit_cooldown(response.headers(), None, config);
            return (response, cooldown);
        }
        let (parts, body_bytes) = collect_response_body(response).await;
        let cooldown = rate_limit_cooldown(
            &parts.headers,
            Some(body_bytes.as_ref()),
            config,
        );
        let response = Response::from_parts(
            parts,
            axum_core::body::Body::from(body_bytes),
        );
        return (response, cooldown);
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
        let response = Response::from_parts(
            parts,
            axum_core::body::Body::from(body_bytes),
        );
        return (response, cooldown);
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
        let response = Response::from_parts(
            parts,
            axum_core::body::Body::from(body_bytes),
        );
        return (response, cooldown);
    }

    (response, config.provider_error)
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
        let (_, cooldown) = cooldown_for_response(response, &config).await;
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
        let (_, cooldown) = cooldown_for_response(response, &config).await;
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
        let (_, cooldown) = cooldown_for_response(response, &config).await;
        assert_eq!(cooldown, config.provider_error + config.retry_after_buffer);
        assert_eq!(cooldown, Duration::from_secs(60 + 1));
    }
}
