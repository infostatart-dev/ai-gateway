use bytes::Bytes;
use http::StatusCode;
use serde_json::Value;

use super::{
    attempt::{classify_outcome, resolve_usage},
    runtime::AttemptRecord,
};
use crate::{
    metrics::llm::TokenUsage,
    router::{
        retry_after::FailoverClass,
        token_estimate::{PayloadBudgetConfig, estimate_from_value},
    },
    types::{
        extensions::{RequestKind, UpstreamAttemptContext},
        model_id::ModelId,
        provider::InferenceProvider,
        router::RouterId,
    },
};

pub struct RecordAttemptInput<'a> {
    pub provider: &'a InferenceProvider,
    pub credential: &'a str,
    pub model: Option<&'a ModelId>,
    pub router_id: Option<&'a RouterId>,
    pub attempt: Option<&'a UpstreamAttemptContext>,
    pub status: StatusCode,
    pub stream: bool,
    pub request_kind: RequestKind,
    pub duration_ms: f64,
    pub tfft_ms: Option<f64>,
    pub reported_usage: TokenUsage,
    pub request_body: Option<&'a Bytes>,
    pub estimate_tokens: bool,
    pub failover_class: Option<FailoverClass>,
}

#[must_use]
pub fn build_attempt_record(input: &RecordAttemptInput<'_>) -> AttemptRecord {
    let estimate = input.request_body.and_then(|body| {
        serde_json::from_slice::<Value>(body).ok().and_then(|v| {
            estimate_from_value(&v, PayloadBudgetConfig::default())
        })
    });
    let (usage, usage_source) = resolve_usage(
        input.reported_usage,
        estimate.map(|e| u64::from(e.input_tokens)),
        estimate.map(|e| u64::from(e.reserved_output)),
        input.estimate_tokens,
    );
    let overload = input.failover_class == Some(FailoverClass::Overload);
    let outcome = classify_outcome(input.status, usage_source, overload);
    AttemptRecord {
        provider: input.provider.to_string(),
        credential: input.credential.to_string(),
        model: input
            .model
            .map_or_else(|| "unknown".to_string(), ToString::to_string),
        router_id: input
            .router_id
            .map_or_else(|| "none".to_string(), ToString::to_string),
        attempt_index: input.attempt.map_or(0, |a| a.attempt_index),
        upstream_attempts: input
            .attempt
            .map_or(1, |a| a.upstream_attempts.max(1)),
        status_code: input.status.as_u16(),
        stream: input.stream,
        request_kind: request_kind_label(input.request_kind),
        duration_ms: input.duration_ms,
        tfft_ms: input.tfft_ms,
        usage,
        usage_source,
        outcome,
        overload,
    }
}

const fn request_kind_label(kind: RequestKind) -> &'static str {
    match kind {
        RequestKind::Router => "router",
        RequestKind::UnifiedApi => "unified_api",
        RequestKind::DirectProxy => "direct_proxy",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::provider::attempt::{CallOutcome, UsageSource};

    #[test]
    fn estimated_usage_on_success_without_provider_usage() {
        let body =
            Bytes::from(r#"{"messages":[{"role":"user","content":"hi"}]}"#);
        let record = build_attempt_record(&RecordAttemptInput {
            provider: &InferenceProvider::OpenAI,
            credential: "default",
            model: None,
            router_id: None,
            attempt: None,
            status: StatusCode::OK,
            stream: false,
            request_kind: RequestKind::DirectProxy,
            duration_ms: 100.0,
            tfft_ms: None,
            reported_usage: TokenUsage::default(),
            request_body: Some(&body),
            estimate_tokens: true,
            failover_class: None,
        });
        assert_eq!(record.usage_source, UsageSource::Estimated);
        assert_eq!(record.outcome, CallOutcome::SuccessDegraded);
        assert!(record.usage.input.unwrap_or(0) > 0);
    }
}
