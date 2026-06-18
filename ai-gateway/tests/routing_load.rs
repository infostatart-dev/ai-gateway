//! Routing load verification integration tests.

use ai_gateway::routing_load::scenarios;

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
routing_load_test!(payload_filter_under_load, scenarios::payload_filter::run);
routing_load_test!(failover_rpm_sibling, scenarios::failover_rpm::run);
routing_load_test!(failover_daily_quota, scenarios::failover_quota::run);
routing_load_test!(
    cloudflare_daily_pacing_gate,
    scenarios::failover_daily_quota::run
);
routing_load_test!(chatgpt_last_resort, scenarios::chatgpt_last_resort::run);
routing_load_test!(pacing_burst, scenarios::pacing_burst::run);
routing_load_test!(shaper_backpressure, scenarios::shaper_backpressure::run);
routing_load_test!(harness_round_robin, scenarios::harness_round_robin::run);
routing_load_test!(
    harness_payload_filter,
    scenarios::harness_payload_filter::run
);
