use std::{collections::HashMap, time::Duration};

use ai_gateway::{
    config::{
        Config,
        balance::{BalanceConfig, BalanceConfigInner},
        decision::{RouterDecisionConfig, TierCascade},
        helicone::HeliconeFeatures,
        router::{RouterConfig, RouterConfigs, SourceModelSelection},
    },
    endpoints::EndpointType,
    tests::{TestDefault, harness::Harness, mock::MockArgs},
    types::{provider::InferenceProvider, router::RouterId},
};
use compact_str::CompactString;
use http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use indexmap::IndexMap;
use nonempty_collections::nes;
use serde_json::Value;
use tower::Service;

use crate::rl::support::*;

async fn fetch_stats(harness: &mut Harness) -> Value {
    let request = Request::builder()
        .method(Method::GET)
        .uri("http://router.helicone.com/v1/observability/provider-stats")
        .body(axum_core::body::Body::empty())
        .unwrap();
    let response = harness.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap();
    serde_json::from_slice(&body.to_bytes()).unwrap()
}

fn gemini_router_config() -> RouterConfigs {
    RouterConfigs::new(HashMap::from([(
        RouterId::Named(CompactString::new("routing-load")),
        RouterConfig {
            load_balance: BalanceConfig(HashMap::from([(
                EndpointType::Chat,
                BalanceConfigInner::BudgetAwareCapabilityAfter {
                    providers: nes![InferenceProvider::GoogleGemini],
                    provider_priorities: IndexMap::from([(
                        InferenceProvider::GoogleGemini,
                        0,
                    )]),
                    max_cooldown_wait: Duration::from_secs(0),
                },
            )])),
            decision: RouterDecisionConfig {
                enabled: true,
                tier_cascade: Some(TierCascade::FreeUp),
            },
            source_model_selection: Some(SourceModelSelection::Intent),
            ..Default::default()
        },
    )]))
}

pub async fn run() {
    prepare_harness_test();
    let harness_secrets = RoutingLoadHarness::gemini_free_only(4);
    let mut config = Config::test_default();
    harness_secrets.apply_credentials(&mut config);
    config.helicone.features = HeliconeFeatures::None;
    config.routers = gemini_router_config();
    let mock_args = MockArgs::builder()
        .stubs(HashMap::from([
            ("success:gemini:generate_content", (32..).into()),
            ("success:minio:upload_request", 0.into()),
            ("success:jawn:log_request", 0.into()),
        ]))
        .verify(false)
        .build();
    let mut harness = Harness::builder()
        .with_config(config)
        .with_mock_args(mock_args)
        .build()
        .await;
    let body = large_chat_body(GROQ_FILTER_EXTRA_CHARS);
    for index in 0..32 {
        let request = Request::builder()
            .method(Method::POST)
            .uri("http://router.helicone.com/router/routing-load/chat/completions")
            .header("content-type", "application/json")
            .header("x-agent-name", "routing-load")
            .header("x-work-unit-id", format!("unit-{index}"))
            .body(axum_core::body::Body::from(body.to_vec()))
            .unwrap();
        let response = harness.call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    let snapshot = fetch_stats(&mut harness).await;
    let slots = [
        "gemini-free",
        "gemini-free-2",
        "gemini-free-3",
        "gemini-free-4",
    ];
    let mut used = 0usize;
    let mut total = 0u64;
    for id in slots {
        let attempts = attempts_for_credential(&snapshot, id);
        total += attempts;
        if attempts > 0 {
            used += 1;
        }
    }
    assert_eq!(total, 32, "expected 32 upstream attempts across slots");
    assert!(
        used >= 2,
        "expected work-unit hash spread across ≥2 credentials (v1 planner), \
         snapshot={snapshot:?}"
    );
    assert_zero_attempts(&snapshot, "chatgpt-web-default");
    assert!(failover_rate(&snapshot) < 0.01);
}
