use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use ai_gateway::{
    app_state::AppState,
    config::client_access::{
        ClientAccessLimitsConfig, ClientAccessWindowLimitsConfig,
    },
    tests::routing::{
        RequestRequirements, clear_test_call_responses, empty_router,
        install_upstream_mock, named_model_candidate,
    },
    types::{extensions::ClientAccessContext, org::OrgId, user::UserId},
};
use gateway_tests::{UpstreamMockScript, upstream::ok_chat_completion};
use tracing::{
    Subscriber,
    field::{Field, Visit},
    instrument::WithSubscriber,
    span::{Attributes, Id},
};
use tracing_subscriber::{
    Layer,
    layer::{Context, SubscriberExt},
    registry::LookupSpan,
};
use uuid::Uuid;

use crate::rl::support::*;

#[derive(Clone, Default)]
struct RouteSpanCapture {
    spans: Arc<Mutex<Vec<HashMap<String, String>>>>,
}

impl RouteSpanCapture {
    fn route_spans(&self) -> Vec<HashMap<String, String>> {
        self.spans.lock().expect("span capture").clone()
    }
}

impl<S> Layer<S> for RouteSpanCapture
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attrs: &Attributes<'_>,
        _id: &Id,
        _ctx: Context<'_, S>,
    ) {
        if attrs.metadata().name() != "gateway.route" {
            return;
        }
        let mut visitor = FieldCapture::default();
        attrs.record(&mut visitor);
        self.spans
            .lock()
            .expect("span capture")
            .push(visitor.fields);
    }
}

#[derive(Default)]
struct FieldCapture {
    fields: HashMap<String, String>,
}

impl FieldCapture {
    fn insert(&mut self, field: &Field, value: impl ToString) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }
}

impl Visit for FieldCapture {
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.insert(field, value);
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.insert(field, value);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.insert(field, value);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.insert(field, value);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.insert(field, format!("{value:?}"));
    }
}

fn client_access_context() -> ClientAccessContext {
    ClientAccessContext {
        key_id: "key-client-alpha".to_string(),
        subject_id: "client-alpha".to_string(),
        user_id: UserId::new(Uuid::nil()),
        org_id: OrgId::new(Uuid::nil()),
        plan_id: "plan-standard".to_string(),
        max_output_tokens: 4_000,
        scopes: Vec::new(),
        quota_limits: ClientAccessLimitsConfig {
            requests: ClientAccessWindowLimitsConfig {
                per_minute: Some(60),
                per_day: Some(10_000),
                per_week: None,
            },
            tokens: ClientAccessWindowLimitsConfig {
                per_minute: Some(1_000_000),
                per_day: Some(100_000_000),
                per_week: None,
            },
        },
    }
}

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new().default_response(ok_chat_completion),
    );

    let capture = RouteSpanCapture::default();
    let subscriber = tracing_subscriber::registry().with(capture.clone());

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let candidate = named_model_candidate(
        &app_state,
        "vllm",
        "vllm-anonymous",
        "am-thinking-awq",
        128_000,
    )
    .await;
    let mut parts = caller_parts("client-alpha-agent", Some("session-42"));
    parts.extensions.insert(client_access_context());

    let result = run_planned_failover(
        router,
        parts,
        default_fat_body(),
        vec![candidate],
        RequestRequirements::default(),
        None,
    )
    .with_subscriber(subscriber)
    .await
    .expect("tagged route should complete");

    assert_eq!(result.response.status(), http::StatusCode::OK);
    let spans = capture.route_spans();
    assert_eq!(spans.len(), 1, "expected one gateway.route span");
    let route = &spans[0];
    assert_eq!(route.get("agent_name"), Some(&"client-alpha-agent".into()));
    assert_eq!(route.get("work_unit_id"), Some(&"session-42".into()));
    assert_eq!(route.get("work_unit_source"), Some(&"explicit".into()));
    assert_eq!(route.get("client_subject_id"), Some(&"client-alpha".into()));
    assert_eq!(route.get("client_key_id"), Some(&"key-client-alpha".into()));
    assert_eq!(route.get("client_plan_id"), Some(&"plan-standard".into()));
}
