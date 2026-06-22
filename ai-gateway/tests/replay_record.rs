//! Route replay log contract (D19).

use ai_gateway::{
    metrics::provider::build_replay_record,
    types::{
        extensions::{
            BlockedReason, PendingRouteTrace, PlanReplaySnapshot,
            ReplayQuotaExcluded, ReplayScoreBreakdown, WorkUnitSource,
        },
        provider::InferenceProvider,
        router::RouterId,
    },
};
use compact_str::CompactString;

fn sample_snapshot() -> PlanReplaySnapshot {
    PlanReplaySnapshot {
        plan_snapshot_ts: "2026-06-18T12:00:00Z".to_string(),
        winner_credential: "gemini-free-9".to_string(),
        winner_model: "gemini-3.1-flash-lite".to_string(),
        winner: ReplayScoreBreakdown {
            score: 0.82,
            h_success: 0.9,
            quota_capacity: 0.7,
            q_cooldown_secs: 0.0,
            m_affinity: 1.0,
            hash_bias: 0.42,
            l_band: 2,
            cost_class: "free".to_string(),
            blocked_reason: None,
            next_available_at: None,
        },
        top_alternatives: vec![],
        quota_excluded: vec![],
    }
}

#[test]
fn replay_record_serializes_winner_breakdown() {
    let pending = PendingRouteTrace {
        router_id: RouterId::Named(CompactString::new("autodefault")),
        strategy: "budget-aware-capability-after",
        hops: 1,
        candidates: 2,
        skipped: 0,
        outcome_label: "success",
        terminal_provider: Some(InferenceProvider::GoogleGemini),
        terminal_credential: Some("gemini-free-9".to_string()),
        terminal_status: Some(200),
        deepseek_web: None,
        chatgpt_web: None,
        intent_tier: None,
        selection_phase: None,
        quota_scope: None,
        model_ladder_band: None,
        model_ladder_position: None,
        upstream_failure_kind: None,
        restricted_until: None,
        failover_class: None,
        agent_name: Some("invoker-alpha".to_string()),
        work_unit_id: Some("unit-1".to_string()),
        work_unit_source: Some(WorkUnitSource::Explicit),
        planned_hops: Some(3),
        plan_rebuilds: Some(0),
        route_memory_hit: Some(true),
        route_memory_invalidated: Some(false),
        source_model: Some("gpt-5-nano".to_string()),
        json_schema_required: true,
        replay: Some(sample_snapshot()),
    };

    let replay = build_replay_record(&pending).expect("replay");
    let json = serde_json::to_value(&replay).unwrap();
    assert_eq!(json["agent_name"], "invoker-alpha");
    assert_eq!(json["plan_snapshot_ts"], "2026-06-18T12:00:00Z");
    assert_eq!(json["winner_credential"], "gemini-free-9");
    assert_eq!(json["winner_score"]["quota_capacity"], 0.7);
    assert_eq!(json["winner_score"]["h_success"], 0.9);
    assert_eq!(json["json_schema_required"], true);
    assert_eq!(json["route_memory_hit"], true);
    assert!(json["winner_score"].get("blocked_reason").is_none());
}

#[test]
fn replay_record_serializes_quota_block_metadata() {
    let pending = PendingRouteTrace {
        router_id: RouterId::Named(CompactString::new("autodefault")),
        strategy: "budget-aware-capability-after",
        hops: 1,
        candidates: 2,
        skipped: 0,
        outcome_label: "terminal_failure",
        terminal_provider: None,
        terminal_credential: None,
        terminal_status: Some(503),
        deepseek_web: None,
        chatgpt_web: None,
        intent_tier: None,
        selection_phase: None,
        quota_scope: None,
        model_ladder_band: None,
        model_ladder_position: None,
        upstream_failure_kind: None,
        restricted_until: None,
        failover_class: None,
        agent_name: Some("invoker-alpha".to_string()),
        work_unit_id: Some("unit-2".to_string()),
        work_unit_source: Some(WorkUnitSource::Explicit),
        planned_hops: Some(1),
        plan_rebuilds: Some(0),
        route_memory_hit: Some(false),
        route_memory_invalidated: Some(false),
        source_model: Some("gpt-5-nano".to_string()),
        json_schema_required: false,
        replay: Some(PlanReplaySnapshot {
            plan_snapshot_ts: "2026-06-18T12:00:00Z".to_string(),
            winner_credential: "gemini-free-9".to_string(),
            winner_model: "gemini-3.1-flash-lite".to_string(),
            winner: ReplayScoreBreakdown {
                score: 0.0,
                h_success: 0.9,
                quota_capacity: 0.0,
                q_cooldown_secs: 12.0,
                m_affinity: 0.0,
                hash_bias: 0.0,
                l_band: 2,
                cost_class: "free".to_string(),
                blocked_reason: Some(BlockedReason::Rpm),
                next_available_at: Some("2026-06-18T12:00:30Z".to_string()),
            },
            top_alternatives: vec![],
            quota_excluded: vec![ReplayQuotaExcluded {
                credential: "gemini-free-3".to_string(),
                model: "gemini-3-flash-preview".to_string(),
                blocked_reason: BlockedReason::Rpm,
                next_available_at: Some("2026-06-18T12:00:45Z".to_string()),
                quota_capacity: 0.0,
            }],
        }),
    };

    let replay = build_replay_record(&pending).expect("replay");
    let json = serde_json::to_value(&replay).unwrap();
    assert_eq!(json["winner_score"]["blocked_reason"], "rpm");
    assert_eq!(
        json["winner_score"]["next_available_at"],
        "2026-06-18T12:00:30Z"
    );
    assert_eq!(json["quota_excluded"][0]["credential"], "gemini-free-3");
    assert_eq!(json["quota_excluded"][0]["blocked_reason"], "rpm");
    assert_eq!(json["quota_excluded"][0]["quota_capacity"], 0.0);
}

#[test]
fn replay_record_absent_without_plan_snapshot() {
    let pending = PendingRouteTrace {
        router_id: RouterId::Named(CompactString::new("autodefault")),
        strategy: "budget-aware-capability-after",
        hops: 1,
        candidates: 1,
        skipped: 0,
        outcome_label: "success",
        terminal_provider: None,
        terminal_credential: None,
        terminal_status: Some(200),
        deepseek_web: None,
        chatgpt_web: None,
        intent_tier: None,
        selection_phase: None,
        quota_scope: None,
        model_ladder_band: None,
        model_ladder_position: None,
        upstream_failure_kind: None,
        restricted_until: None,
        failover_class: None,
        agent_name: Some("invoker".to_string()),
        work_unit_id: None,
        work_unit_source: None,
        planned_hops: None,
        plan_rebuilds: None,
        route_memory_hit: None,
        route_memory_invalidated: None,
        source_model: None,
        json_schema_required: false,
        replay: None,
    };
    assert!(build_replay_record(&pending).is_none());
}
