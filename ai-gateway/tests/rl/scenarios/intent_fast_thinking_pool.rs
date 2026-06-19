use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        IntentTier, balance_ranked, clear_test_call_responses,
        deep_paid_candidate, extract_requirements_from_value,
        extract_routing_intent, extract_source_model_from_value,
        install_upstream_mock, intent_autodefault_router,
        ordered_candidates_for_source, request_parts, run_failover_candidates,
        scout_candidate,
    },
};
use futures::future::join_all;
use gateway_tests::{
    UpstreamMockScript, upstream::ok_nano_json_schema_completion,
};

use crate::rl::support::*;

const SCOUT_SLOTS: [&str; 4] = [
    "groq-scout-1",
    "groq-scout-2",
    "groq-scout-3",
    "groq-scout-4",
];

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new()
            .default_response(ok_nano_json_schema_completion),
    );

    let app_state = AppState::test_default().await;
    let mut candidates = Vec::new();
    for slot in SCOUT_SLOTS {
        candidates.push(scout_candidate(&app_state, slot).await);
    }
    candidates.push(deep_paid_candidate(&app_state).await);

    let router = intent_autodefault_router(&app_state, candidates);
    let body = nano_json_strict_body();
    let parsed: serde_json::Value =
        serde_json::from_slice(&body).expect("body");
    let requirements = extract_requirements_from_value(&parsed);
    let source_model = extract_source_model_from_value(&parsed).expect("model");
    let routing_intent = extract_routing_intent(&source_model);

    let ordered =
        ordered_candidates_for_source(&router, &requirements, &source_model)
            .expect("ordered");
    assert_eq!(
        ordered[0].capability.intent_tier,
        IntentTier::FastThinking,
        "first hop must stay in fast-thinking band"
    );
    assert!(
        ordered
            .iter()
            .filter(|c| c.capability.intent_tier == IntentTier::Deep)
            .count()
            <= 1,
        "deep tier is escalation-only"
    );

    let parts = request_parts();
    let ranked = balance_ranked(&router, ordered);
    let futures = (0..32).map(|_| {
        let router = router.clone();
        let ranked = ranked.clone();
        let parts = parts.clone();
        let body = body.clone();
        let requirements = requirements.clone();
        async move {
            let response = run_failover_candidates(
                router,
                parts,
                body,
                ranked,
                requirements,
                Some(routing_intent),
            )
            .await
            .expect("success");
            routed_identity(&response)
        }
    });
    let identities: Vec<_> = join_all(futures).await;
    let counts = terminal_provider_counts(&identities);
    assert_zero_terminal_credentials(&counts, &["anthropic-test"]);
    let groq_hits: usize = SCOUT_SLOTS
        .iter()
        .map(|slot| counts.get(*slot).copied().unwrap_or(0))
        .sum();
    assert!(
        groq_hits >= 24,
        "expected spread across fast-thinking scout pool, got {counts:?}"
    );
}
