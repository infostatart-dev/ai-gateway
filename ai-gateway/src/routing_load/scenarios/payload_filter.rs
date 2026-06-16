use futures::future::join_all;

use crate::{
    app_state::AppState,
    router::{
        budget_aware::{
            clear_test_call_responses, gemini_slots, groq_candidate,
            ordered_candidates, push_test_call_response_for_credential,
            request_parts, router_with_candidates, run_failover_candidates,
        },
        capability::{apply_payload_estimate, extract_requirements_from_value},
        token_estimate::{PayloadBudgetConfig, estimate_from_value},
    },
    routing_load::{
        assert_identity::{routed_identity, terminal_provider_counts},
        payload::{GROQ_FILTER_EXTRA_CHARS, fat_json_schema_body},
        responses::ok_fat_json_schema_completion,
    },
};

pub async fn run() {
    clear_test_call_responses();
    for _ in 0..16 {
        push_test_call_response_for_credential(
            "gemini-free",
            Ok(ok_fat_json_schema_completion()),
        );
    }

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
