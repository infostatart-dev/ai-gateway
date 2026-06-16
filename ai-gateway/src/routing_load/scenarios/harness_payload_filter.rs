use std::{collections::HashMap, time::Duration};

use compact_str::CompactString;
use http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use indexmap::IndexMap;
use nonempty_collections::nes;
use tower::Service;

use crate::{
    config::{
        Config,
        balance::{BalanceConfig, BalanceConfigInner},
        helicone::HeliconeFeatures,
        router::{RouterConfig, RouterConfigs},
    },
    endpoints::EndpointType,
    routing_load::{
        assert_stats::attempts_for_credential,
        payload::{GROQ_FILTER_EXTRA_CHARS, large_chat_body},
        router::{RoutingLoadHarness, prepare_harness_test},
    },
    tests::{TestDefault, harness::Harness, mock::MockArgs},
    types::{provider::InferenceProvider, router::RouterId},
};

pub async fn run() {
    prepare_harness_test();
    let harness_secrets = RoutingLoadHarness::gemini_free_only(1);
    let path = harness_secrets.secrets_path.join("secrets.yaml");
    let mut yaml = std::fs::read_to_string(&path).expect("read secrets");
    yaml.push_str("  groq-default:\n    api-key: groq-key\n");
    std::fs::write(&path, yaml).expect("write secrets");

    let mut config = Config::test_default();
    harness_secrets.apply_credentials(&mut config);
    config.helicone.features = HeliconeFeatures::None;
    config.routers = RouterConfigs::new(HashMap::from([(
        RouterId::Named(CompactString::new("routing-load")),
        RouterConfig {
            load_balance: BalanceConfig(HashMap::from([(
                EndpointType::Chat,
                BalanceConfigInner::BudgetAwareCapabilityAfter {
                    providers: nes![
                        InferenceProvider::Named("groq".into()),
                        InferenceProvider::GoogleGemini,
                    ],
                    provider_priorities: IndexMap::from([
                        (InferenceProvider::Named("groq".into()), 0),
                        (InferenceProvider::GoogleGemini, 1),
                    ]),
                    max_cooldown_wait: Duration::from_secs(0),
                },
            )])),
            ..Default::default()
        },
    )]));
    let mock_args = MockArgs::builder()
        .stubs(HashMap::from([
            ("success:gemini:generate_content", (8..).into()),
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
    for _ in 0..8 {
        let request = Request::builder()
            .method(Method::POST)
            .uri("http://router.helicone.com/router/routing-load/chat/completions")
            .header("content-type", "application/json")
            .body(axum_core::body::Body::from(body.to_vec()))
            .unwrap();
        let response = harness.call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    let stats_request = Request::builder()
        .method(Method::GET)
        .uri("http://router.helicone.com/v1/observability/provider-stats")
        .body(axum_core::body::Body::empty())
        .unwrap();
    let stats = harness.call(stats_request).await.unwrap();
    let body = stats.into_body().collect().await.unwrap();
    let snapshot: serde_json::Value =
        serde_json::from_slice(&body.to_bytes()).unwrap();
    assert_eq!(attempts_for_credential(&snapshot, "groq-default"), 0);
    assert!(attempts_for_credential(&snapshot, "gemini-free") >= 8);
}
