//! Routing load verification integration tests.

mod rl;

use rl::scenarios;

macro_rules! routing_load_test {
    ($name:ident, $scenario:expr) => {
        #[tokio::test]
        #[serial_test::serial]
        async fn $name() {
            $scenario().await;
        }
    };
}

routing_load_test!(
    intent_fast_thinking_pool,
    scenarios::intent_fast_thinking_pool::run
);
routing_load_test!(round_robin_concurrent, scenarios::round_robin::run);
routing_load_test!(
    gemini_sixteen_slot_concurrent,
    scenarios::gemini_sixteen_slot::run
);
routing_load_test!(
    gemini_model_ladder_same_slot,
    scenarios::gemini_model_ladder_same_slot::run
);
routing_load_test!(
    gemini_stability_escalation,
    scenarios::gemini_stability_escalation::run
);
routing_load_test!(
    gemini_404_retires_model_not_slot,
    scenarios::gemini_404_retires_model_not_slot::run
);
routing_load_test!(
    gemini_503_high_demand_continues_ladder,
    scenarios::gemini_503_high_demand_continues_ladder::run
);
routing_load_test!(
    gemini_stability_escalates_up,
    scenarios::gemini_stability_escalates_up::run
);
routing_load_test!(payload_filter_under_load, scenarios::payload_filter::run);
routing_load_test!(failover_rpm_sibling, scenarios::failover_rpm::run);
routing_load_test!(failover_daily_quota, scenarios::failover_quota::run);
routing_load_test!(
    cloudflare_daily_pacing_gate,
    scenarios::failover_daily_quota::run
);
routing_load_test!(chatgpt_last_resort, scenarios::chatgpt_last_resort::run);
routing_load_test!(
    openrouter_nemotron_429_then_gpt_oss_200,
    scenarios::openrouter_nemotron_429_then_gpt_oss_200::run
);
routing_load_test!(
    openrouter_402_paid_does_not_kill_free,
    scenarios::openrouter_402_paid_does_not_kill_free::run
);
routing_load_test!(pacing_burst, scenarios::pacing_burst::run);
routing_load_test!(
    deepseek_credential_restricted_failover,
    scenarios::deepseek_credential_restricted_failover::run
);
routing_load_test!(
    deepseek_four_slot_partial_restriction,
    scenarios::deepseek_four_slot_partial_restriction::run
);
routing_load_test!(
    deepseek_restricted_then_gemini_stability,
    scenarios::deepseek_restricted_then_gemini_stability::run
);
routing_load_test!(shaper_backpressure, scenarios::shaper_backpressure::run);
routing_load_test!(harness_round_robin, scenarios::harness_round_robin::run);
routing_load_test!(
    harness_payload_filter,
    scenarios::harness_payload_filter::run
);
routing_load_test!(
    credential_circuit_open,
    scenarios::credential_circuit_open::run
);
routing_load_test!(
    caller_request_id_spread,
    scenarios::caller_request_id_spread::run
);
routing_load_test!(
    caller_three_work_units,
    scenarios::caller_three_work_units::run
);
routing_load_test!(route_plan_max_hops, scenarios::route_plan_max_hops::run);
routing_load_test!(
    stability_escalation_plan,
    scenarios::stability_escalation_plan::run
);
routing_load_test!(
    stability_never_downgrade,
    scenarios::stability_never_downgrade::run
);
routing_load_test!(
    dynamic_cooldown_skip,
    scenarios::dynamic_cooldown_skip::run
);
routing_load_test!(
    free_catalog_pacing_skip,
    scenarios::free_catalog_pacing_skip::run
);
routing_load_test!(
    route_memory_sticky_reuse,
    scenarios::route_memory_sticky_reuse::run
);
routing_load_test!(
    route_memory_invalidate_on_429,
    scenarios::route_memory_invalidate_on_429::run
);
routing_load_test!(
    admission_zero_repeat_429,
    scenarios::admission_zero_repeat_429::run
);
routing_load_test!(
    admission_parallel_account_spread,
    scenarios::admission_parallel_account_spread::run
);
routing_load_test!(
    admission_hop_readmit,
    scenarios::admission_hop_readmit::run
);
routing_load_test!(
    admission_longcat_tpd,
    scenarios::admission_longcat_tpd::run
);
routing_load_test!(
    admission_per_session_deepseek,
    scenarios::admission_per_session_deepseek::run
);
routing_load_test!(
    quota_parallel_collision,
    scenarios::quota_parallel_collision::run
);
