//! Shared harness imports for routing load scenarios.

pub use ai_gateway::tests::routing_harness::{
    agent_header,
    assert_identity::{
        assert_fairness_band, assert_zero_terminal_credentials,
        routed_identity, terminal_provider_counts,
    },
    assert_stats::{
        assert_zero_attempts, attempts_for_credential, credential_attempts,
        failover_rate, total_client_requests,
    },
    caller_parts,
    pacing_harness::saturate_model_pacing,
    payload::{
        GROQ_FILTER_EXTRA_CHARS, default_fat_body, fat_json_schema_body,
        large_chat_body, nano_json_strict_body,
    },
    responses::{
        credential_restricted, daily_quota_exhausted, high_demand_503,
        not_found_404, ok_chat_completion, ok_fat_json_schema_completion,
        ok_nano_json_schema_completion, openrouter_free_models_per_day_429,
        openrouter_never_purchased_402, overload_503,
        project_billing_exhausted, rate_limited_rpm,
    },
    router::{RoutingLoadHarness, prepare_harness_test},
    run::{PlannedResponse, run_planned_failover},
    work_unit_header,
};
