use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        PayloadBudgetConfig, apply_payload_estimate, clear_test_call_responses,
        estimate_from_value, extract_requirements_from_value, gemini_slots,
        groq_candidate, install_upstream_mock, ordered_candidates,
        request_parts, router_with_candidates, run_failover_candidates,
    },
};
use futures::future::join_all;
use gateway_tests::{
    UpstreamMockScript, upstream::ok_fat_json_schema_completion,
};

use crate::rl::support::*;

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new()
            .credential("gemini-free", vec![ok_fat_json_schema_completion])
            .default_response(ok_fat_json_schema_completion),
    );

    let app_state = AppState::test_default().await;
    let mut candidates = gemini_slots(&app_state, 1).await;
    candidates.push(groq_candidate(&app_state).await);
    let router = router_with_candidates(&app_state, candidates);
    let body = fat_json_schema_body(GROQ_FILTER_EXTRA_CHARS);
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let budget = PayloadBudgetConfig::default();
    let mut requirements = extract_requirements_from_value(&parsed);
    if let Some(estimate) = estimate_from_value(&parsed, budget) {
        apply_payload_estimate(&mut requirements, estimate);
    }
    let filtered =
        ordered_candidates(&router, &requirements).expect("candidates");
    assert!(
        filtered
            .iter()
            .all(|c| c.credential_id.to_string() != "groq-default"),
        "groq should be payload-filtered"
    );
    assert!(
        filtered
            .iter()
            .any(|c| c.credential_id.to_string().starts_with("gemini-free")),
        "gemini should remain eligible"
    );

    let parts = request_parts();
    let futures = (0..16).map(|_| {
        let router = router.clone();
        let filtered = filtered.clone();
        let parts = parts.clone();
        let body = body.clone();
        let requirements = requirements.clone();
        async move {
            let response = run_failover_candidates(
                router,
                parts,
                body,
                filtered,
                requirements,
                None,
            )
            .await
            .expect("dispatch");
            routed_identity(&response)
        }
    });
    let identities: Vec<_> = join_all(futures).await;
    let counts = terminal_provider_counts(&identities);
    assert_eq!(counts.get("groq-default").copied().unwrap_or(0), 0);
    assert!(
        counts.get("gemini-free").copied().unwrap_or(0) > 0,
        "expected gemini traffic"
    );
}
