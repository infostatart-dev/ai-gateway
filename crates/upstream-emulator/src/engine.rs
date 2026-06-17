use ai_gateway::types::provider::InferenceProvider;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::Value;

use crate::{
    family::ProtocolFamily,
    profiles::ForcedProfile,
    state::SharedState,
    tokens::estimate_usage,
    wire::{
        auth_error_response, overload_response, quota_exhausted_response,
        rate_limit_response, render_api_family,
    },
};

pub async fn dispatch_api_key(
    State(state): State<SharedState>,
    provider: InferenceProvider,
    family: ProtocolFamily,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if method != Method::POST {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let credential = bearer_key(&headers).unwrap_or_else(|| "anonymous".into());
    let scope = format!("{provider}:{credential}");
    if let Some(profile) = state.profiles.forced_profile(&scope) {
        return forced_response(profile);
    }
    let parsed = parse_json_body(&body);
    let model = parsed
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("default");
    let credential_tier = state.tiers.tier_for(&provider, &credential);
    let input_tokens = estimate_input_tokens(&parsed);
    match state.limits.check_api_key(
        &state.catalog,
        &provider,
        credential_tier.as_deref(),
        model,
        &credential,
        input_tokens,
    ) {
        Ok(_lease) => {
            let content = crate::capability::assistant_content(
                &parsed,
                &state.providers,
                &provider,
            );
            let usage = estimate_usage(&parsed, &content);
            state
                .sleep_for_usage(provider.as_ref(), usage.total())
                .await;
            render_api_family(family, &parsed, &state.providers, &provider)
        }
        Err(verdict) => rate_limit_response(family, verdict),
    }
}

fn forced_response(profile: ForcedProfile) -> Response {
    match profile {
        ForcedProfile::AuthError => auth_error_response(),
        ForcedProfile::QuotaExhausted => quota_exhausted_response(),
        ForcedProfile::Overload => overload_response(),
    }
}

fn bearer_key(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_string)
}

fn parse_json_body(body: &Bytes) -> Value {
    serde_json::from_slice(body).unwrap_or(Value::Null)
}

fn estimate_input_tokens(body: &Value) -> u32 {
    crate::tokens::estimate_usage(body, "").prompt_tokens
}
