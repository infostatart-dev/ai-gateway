use futures::future::BoxFuture;
use http::{StatusCode, header};
use http_body_util::BodyExt;
use serde::Serialize;
use serde_json::Value;

use super::{failover_loop, types::BudgetAwareRouter};
use crate::{
    config::credentials::ProviderCredentialId,
    error::{api::ApiError, internal::InternalError},
    router::{
        capability::{
            RequestRequirements, apply_payload_estimate,
            extract_requirements_from_value, extract_source_model_from_value,
        },
        token_estimate::{PayloadBudgetConfig, estimate_from_value},
    },
    types::{
        body::Body, extensions::CallerRequestContext, request::Request,
        response::Response,
    },
};

const MODEL_NOT_DECLARED: &str = "model_not_declared";
const MODEL_NOT_DECLARED_MESSAGE: &str =
    "autodefault accepts only declared gateway bindings from /models";
const MODEL_MISSING: &str = "<missing>";
const MODEL_NON_STRING: &str = "<non-string>";

struct RequestRouteContext {
    requirements: RequestRequirements,
    routing_intent: Option<crate::router::intent::RoutingIntent>,
    source_model: Option<crate::types::model_id::ModelId>,
    source_model_label: Option<String>,
    model_admission: ModelAdmission,
    stream: bool,
    stream_mode: super::RouteStreamMode,
}

enum ModelAdmission {
    String(String),
    Missing,
    NonString,
}

impl ModelAdmission {
    fn rejected_label(&self) -> Option<&str> {
        match self {
            Self::String(model)
                if crate::declared_models::is_declared_gateway_binding(
                    model,
                ) =>
            {
                None
            }
            Self::String(model) => Some(model.as_str()),
            Self::Missing => Some(MODEL_MISSING),
            Self::NonString => Some(MODEL_NON_STRING),
        }
    }
}

pub(super) fn budget_aware_call(
    this: BudgetAwareRouter,
    req: Request,
) -> BoxFuture<'static, Result<Response, ApiError>> {
    Box::pin(async move {
        let (parts, body) = req.into_parts();
        let body_bytes = body
            .collect()
            .await
            .map_err(InternalError::CollectBodyError)?
            .to_bytes();

        let route_ctx = request_route_context(&parts, &body_bytes);
        if let Some(response) =
            model_not_declared_admission_response(&this, &parts, &route_ctx)
        {
            return Ok(response);
        }

        let mut pool = this.ordered_candidates(
            &route_ctx.requirements,
            route_ctx.source_model.as_ref(),
        )?;
        if let Some(credential_id) = managed_credential_id(&parts) {
            pool.retain(|candidate| &candidate.credential_id == credential_id);
        }

        let caller = parts
            .extensions
            .get::<CallerRequestContext>()
            .cloned()
            .unwrap_or_else(|| {
                let (work_unit_id, work_unit_source) =
                    crate::middleware::caller_context::resolve_work_unit(
                        &http::HeaderMap::new(),
                    );
                CallerRequestContext {
                    agent_name:
                        crate::middleware::caller_context::DEFAULT_AGENT_NAME
                            .to_string(),
                    work_unit_id: Some(work_unit_id),
                    work_unit_source,
                }
            });
        let estimated_tokens =
            route_ctx.requirements.min_context_tokens.unwrap_or(0);
        let plan = super::plan::plan_route_chain(
            &this,
            pool.clone(),
            &route_ctx.requirements,
            route_ctx.routing_intent,
            &caller,
            this.app_state.credential_health(),
            this.app_state.route_memory(),
            estimated_tokens,
            &std::collections::HashSet::new(),
            route_ctx.source_model_label.as_deref(),
            route_ctx.stream_mode,
        )
        .await;
        if plan.chain.is_empty() {
            if !pool.is_empty() {
                return Ok(failover_loop::route_exhausted_response(
                    std::time::Duration::from_secs(1),
                ));
            }
            return Err(ApiError::Internal(InternalError::ProviderNotFound));
        }
        let candidates = plan.chain;
        let mut parts = parts;
        parts
            .extensions
            .insert(crate::types::extensions::RoutePlanContext {
                caller: caller.clone(),
                full_pool: pool,
                estimated_tokens,
                route_memory_key: plan.memory_key.clone(),
                route_memory_hit: plan.route_memory_hit,
                route_memory_hit_binding: plan.route_memory_hit_binding,
                planned_hops: plan.planned_hops,
                source_model: route_ctx.source_model_label,
                stream: route_ctx.stream,
                json_schema_required: route_ctx
                    .requirements
                    .json_schema_required,
                replay: plan.replay,
            });

        failover_loop::run_failover_candidates(
            this,
            parts,
            body_bytes,
            candidates,
            route_ctx.requirements,
            route_ctx.routing_intent,
        )
        .await
    })
}

fn model_not_declared_admission_response(
    this: &BudgetAwareRouter,
    parts: &http::request::Parts,
    route_ctx: &RequestRouteContext,
) -> Option<Response> {
    if !is_autodefault_router(this) {
        return None;
    }
    let requested_model = route_ctx.model_admission.rejected_label()?;

    let span = model_not_declared_span(this, parts, route_ctx, requested_model);
    let _entered = span.enter();
    Some(model_not_declared_response(
        this,
        parts,
        route_ctx,
        requested_model,
    ))
}

fn model_not_declared_response(
    this: &BudgetAwareRouter,
    parts: &http::request::Parts,
    route_ctx: &RequestRouteContext,
    requested_model: &str,
) -> Response {
    let mut response = http::Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
        .body(Body::from(
            serde_json::to_vec(&ModelNotDeclaredResponse {
                error: ModelNotDeclaredError {
                    r#type: MODEL_NOT_DECLARED,
                    message: MODEL_NOT_DECLARED_MESSAGE,
                    requested_model,
                },
            })
            .expect("model_not_declared response serializes"),
        ))
        .expect("model_not_declared response builds");

    response.extensions_mut().insert(
        crate::types::extensions::GatewayFailureContext {
            failure_stage: "admission",
            error_source: "gateway",
            error_class: MODEL_NOT_DECLARED.to_string(),
        },
    );
    response.extensions_mut().insert(
        crate::types::extensions::PendingRouteTrace {
            router_id: this.router_id.clone(),
            strategy: this.strategy,
            hops: 0,
            candidates: 0,
            skipped: 0,
            outcome_label: "admission_rejected",
            terminal_provider: None,
            terminal_credential: None,
            terminal_status: Some(StatusCode::BAD_REQUEST.as_u16()),
            deepseek_web: None,
            chatgpt_web: None,
            intent_tier: route_ctx
                .routing_intent
                .map(|intent| intent.preferred_tier),
            selection_phase: None,
            quota_scope: None,
            model_ladder_band: None,
            model_ladder_position: None,
            upstream_failure_kind: None,
            restricted_until: None,
            failover_class: None,
            failure_stage: Some("admission".to_string()),
            error_source: Some("gateway".to_string()),
            error_class: Some(MODEL_NOT_DECLARED.to_string()),
            agent_name: parts
                .extensions
                .get::<CallerRequestContext>()
                .map(|ctx| ctx.agent_name.clone()),
            work_unit_id: parts
                .extensions
                .get::<CallerRequestContext>()
                .and_then(|ctx| ctx.work_unit_id.clone()),
            work_unit_source: parts
                .extensions
                .get::<CallerRequestContext>()
                .map(|ctx| ctx.work_unit_source),
            planned_hops: Some(0),
            plan_rebuilds: Some(0),
            route_memory_hit: Some(false),
            route_memory_invalidated: Some(false),
            summary: crate::types::extensions::RouteTraceSummary::default(),
            source_model: Some(requested_model.to_string()),
            json_schema_required: route_ctx.requirements.json_schema_required,
            estimated_usage: crate::metrics::llm::TokenUsage::default(),
            replay: None,
            finalize: None,
        },
    );

    tracing::warn!(
        source_model = requested_model,
        error.type = MODEL_NOT_DECLARED,
        "autodefault rejected undeclared model"
    );

    response
}

fn model_not_declared_span(
    this: &BudgetAwareRouter,
    parts: &http::request::Parts,
    route_ctx: &RequestRouteContext,
    requested_model: &str,
) -> tracing::Span {
    let caller = parts.extensions.get::<CallerRequestContext>();
    tracing::info_span!(
        "gateway.route",
        router_id = %this.router_id,
        strategy = this.strategy,
        agent_name = caller.map_or("none", |ctx| ctx.agent_name.as_str()),
        work_unit_id = caller
            .and_then(|ctx| ctx.work_unit_id.as_deref())
            .unwrap_or("none"),
        work_unit_source = caller.map_or("none", |ctx| match ctx.work_unit_source {
            crate::types::extensions::WorkUnitSource::Explicit => "explicit",
            crate::types::extensions::WorkUnitSource::HeliconeSession => "helicone-session",
            crate::types::extensions::WorkUnitSource::RequestId => "request-id",
            crate::types::extensions::WorkUnitSource::Generated => "generated",
        }),
        source_model = requested_model,
        candidates = 0usize,
        planned_hops = 0u32,
        plan_rebuilds = 0u32,
        route_memory_hit = false,
        route_memory_invalidated = false,
        route_memory_hit_binding = "none",
        route_memory_penalized_binding = "none",
        route_memory_recorded_binding = "none",
        route_memory_policy = "none",
        json_schema_required = route_ctx.requirements.json_schema_required,
        attempts_total = 0u32,
        failover_count = 0u32,
        failed_attempts_total = 0u32,
        attempt_statuses = "",
        attempt_error_classes = "",
        last_failover_class = "none",
        last_failover_error_class = "none",
        last_failed_provider = "none",
        last_failed_credential = "none",
        last_failed_model = "none",
        terminal_provider = tracing::field::Empty,
        terminal_credential = tracing::field::Empty,
        terminal_model = tracing::field::Empty,
        terminal_status = StatusCode::BAD_REQUEST.as_u16(),
        terminal_outcome = "admission_rejected",
        terminal_error_class = MODEL_NOT_DECLARED,
    )
}

#[derive(Serialize)]
struct ModelNotDeclaredResponse<'a> {
    error: ModelNotDeclaredError<'a>,
}

#[derive(Serialize)]
struct ModelNotDeclaredError<'a> {
    r#type: &'static str,
    message: &'static str,
    requested_model: &'a str,
}

fn request_route_context(
    parts: &http::request::Parts,
    body_bytes: &bytes::Bytes,
) -> RequestRouteContext {
    let parsed: Option<Value> = serde_json::from_slice(body_bytes).ok();
    let mut requirements = parsed
        .as_ref()
        .map(extract_requirements_from_value)
        .unwrap_or_default();
    if let Some(value) = parsed.as_ref()
        && let Some(estimate) =
            estimate_from_value(value, PayloadBudgetConfig::default())
    {
        apply_payload_estimate(&mut requirements, estimate);
    }
    let source_model = parsed.as_ref().and_then(|value| {
        extract_managed_source_model(parts, value)
            .or_else(|| extract_source_model_from_value(value))
    });
    let routing_intent = source_model
        .as_ref()
        .map(crate::router::intent::extract_routing_intent);
    let source_model_label = parsed
        .as_ref()
        .and_then(|value| value.get("model").and_then(Value::as_str))
        .map(ToString::to_string);
    let model_admission = model_admission(parsed.as_ref());
    let stream = super::structured_output::request_is_stream(body_bytes);
    let stream_mode = if stream {
        super::RouteStreamMode::Streaming
    } else {
        super::RouteStreamMode::NonStreaming
    };

    RequestRouteContext {
        requirements,
        routing_intent,
        source_model,
        source_model_label,
        model_admission,
        stream,
        stream_mode,
    }
}

fn model_admission(parsed: Option<&Value>) -> ModelAdmission {
    let Some(value) = parsed else {
        return ModelAdmission::Missing;
    };
    let Some(model) = value.get("model") else {
        return ModelAdmission::Missing;
    };
    match model.as_str() {
        Some(model) => ModelAdmission::String(model.to_string()),
        None => ModelAdmission::NonString,
    }
}

fn managed_credential_id(
    parts: &http::request::Parts,
) -> Option<&ProviderCredentialId> {
    if parts
        .extensions
        .get::<crate::types::extensions::RequestKind>()
        != Some(&crate::types::extensions::RequestKind::Managed)
    {
        return None;
    }
    parts.extensions.get::<ProviderCredentialId>()
}

fn is_autodefault_router(this: &BudgetAwareRouter) -> bool {
    this.router_id == crate::config::Config::autodefault_router_id()
}

fn extract_managed_source_model(
    parts: &http::request::Parts,
    value: &Value,
) -> Option<crate::types::model_id::ModelId> {
    if parts
        .extensions
        .get::<crate::types::extensions::RequestKind>()
        != Some(&crate::types::extensions::RequestKind::Managed)
    {
        return None;
    }
    let provider = parts
        .extensions
        .get::<crate::types::provider::InferenceProvider>()?;
    let model = value.get("model").and_then(Value::as_str)?;
    crate::types::model_id::ModelId::from_str_and_provider(
        provider.clone(),
        model
            .strip_prefix(provider.as_ref())
            .and_then(|rest| rest.strip_prefix('/'))
            .unwrap_or(model),
    )
    .ok()
}

#[cfg(all(test, feature = "testing"))]
mod tests {
    use axum_core::body::Body;
    use http::StatusCode;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;
    use crate::{
        app_state::AppState,
        router::budget_aware::{
            clear_test_call_responses, gemini_model_candidate,
            intent_autodefault_router, push_test_call_response,
        },
    };

    async fn router() -> BudgetAwareRouter {
        let app_state = AppState::test_default().await;
        let candidate = gemini_model_candidate(
            &app_state,
            "gemini-free-test",
            "gemini-3.1-flash-lite",
        )
        .await;
        intent_autodefault_router(&app_state, vec![candidate])
    }

    fn request(model: &str) -> Request {
        request_body(&format!(
            r#"{{"model":"{model}","messages":[{{"role":"user","content":"hi"}}]}}"#
        ))
    }

    fn request_body(body: &str) -> Request {
        http::Request::builder()
            .uri("/router/autodefault/chat/completions")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    fn ok_response() -> Response {
        http::Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(r#"{"id":"ok","choices":[]}"#))
            .unwrap()
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn autodefault_accepts_declared_binding() {
        clear_test_call_responses();
        push_test_call_response(Ok(ok_response()));

        let response = router()
            .await
            .oneshot(request("openai/gpt-5.5-mini"))
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let pending = response
            .extensions()
            .get::<crate::types::extensions::PendingRouteTrace>()
            .expect("route trace");
        assert_eq!(
            pending.source_model.as_deref(),
            Some("openai/gpt-5.5-mini")
        );
        clear_test_call_responses();
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn autodefault_rejects_glm_free_slug_before_provider_attempt() {
        rejects_undeclared_model_before_provider_attempt("glm-4.5-air:free")
            .await;
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn autodefault_rejects_openrouter_free_slug_before_provider_attempt()
    {
        rejects_undeclared_model_before_provider_attempt(
            "openrouter/openrouter/free",
        )
        .await;
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn autodefault_rejects_missing_model_before_provider_attempt() {
        rejects_invalid_model_body_before_provider_attempt(
            r#"{"messages":[{"role":"user","content":"hi"}]}"#,
            MODEL_MISSING,
        )
        .await;
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn autodefault_rejects_non_string_model_before_provider_attempt() {
        rejects_invalid_model_body_before_provider_attempt(
            r#"{"model":42,"messages":[{"role":"user","content":"hi"}]}"#,
            MODEL_NON_STRING,
        )
        .await;
    }

    async fn rejects_undeclared_model_before_provider_attempt(model: &str) {
        clear_test_call_responses();
        push_test_call_response(Ok(ok_response()));

        let mut response = router()
            .await
            .oneshot(request(model))
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let pending = response
            .extensions()
            .get::<crate::types::extensions::PendingRouteTrace>()
            .expect("route trace");
        assert_eq!(pending.hops, 0);
        assert_eq!(pending.candidates, 0);
        assert_eq!(pending.source_model.as_deref(), Some(model));
        assert_eq!(pending.error_class.as_deref(), Some(MODEL_NOT_DECLARED));
        assert_eq!(pending.terminal_provider, None);
        assert_eq!(pending.terminal_credential, None);

        let body = response
            .body_mut()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let json: serde_json::Value =
            serde_json::from_slice(&body).expect("json");
        assert_eq!(json["error"]["type"], MODEL_NOT_DECLARED);
        assert_eq!(json["error"]["message"], MODEL_NOT_DECLARED_MESSAGE);
        assert_eq!(json["error"]["requested_model"], model);

        let second = router()
            .await
            .oneshot(request("openai/gpt-5.5-mini"))
            .await
            .expect("second response");
        assert_eq!(
            second.status(),
            StatusCode::OK,
            "rejected request must not consume queued upstream response"
        );
        clear_test_call_responses();
    }

    async fn rejects_invalid_model_body_before_provider_attempt(
        body: &str,
        expected_model: &str,
    ) {
        clear_test_call_responses();
        push_test_call_response(Ok(ok_response()));

        let mut response = router()
            .await
            .oneshot(request_body(body))
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let pending = response
            .extensions()
            .get::<crate::types::extensions::PendingRouteTrace>()
            .expect("route trace");
        assert_eq!(pending.hops, 0);
        assert_eq!(pending.candidates, 0);
        assert_eq!(pending.source_model.as_deref(), Some(expected_model));
        assert_eq!(pending.error_class.as_deref(), Some(MODEL_NOT_DECLARED));

        let body = response
            .body_mut()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let json: serde_json::Value =
            serde_json::from_slice(&body).expect("json");
        assert_eq!(json["error"]["type"], MODEL_NOT_DECLARED);
        assert_eq!(json["error"]["requested_model"], expected_model);

        let second = router()
            .await
            .oneshot(request("openai/gpt-5.5-mini"))
            .await
            .expect("second response");
        assert_eq!(
            second.status(),
            StatusCode::OK,
            "rejected request must not consume queued upstream response"
        );
        clear_test_call_responses();
    }
}
