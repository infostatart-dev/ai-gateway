//! Per-request routing trace summary emitted once at the end of failover so ops
//! can see hops, duration and the terminal outcome without grepping every hop.

use std::time::Instant;

use super::trace_context::{TerminalRouteContext, terminal_route_context};
use crate::{
    config::{
        credentials::ProviderCredentialId,
        provider_limits::ProviderLimitCatalog,
    },
    router::budget_aware::types::BudgetCandidate,
    types::{
        extensions::PendingRouteTrace, provider::InferenceProvider,
        router::RouterId,
    },
};

/// `DeepSeek` Web multi-turn execution stats attached to dispatch responses.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeepSeekWebTrace {
    pub turns: u32,
    pub upload_parts: u32,
    pub pow_cache_hits: u32,
}

/// `ChatGPT` Web multi-turn execution stats attached to dispatch responses.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChatGptWebTrace {
    pub turns: u32,
    pub upload_parts: u32,
}

/// The terminal upstream a request settled on (or none).
pub(super) struct RouteOutcome<'a> {
    pub label: &'static str,
    pub provider: Option<&'a InferenceProvider>,
    pub credential: Option<&'a ProviderCredentialId>,
    pub status: Option<u16>,
}

pub(super) struct RouteTrace {
    _started: Instant,
    candidates: usize,
    attempts: u32,
    skipped: usize,
    deepseek_web: Option<DeepSeekWebTrace>,
    chatgpt_web: Option<ChatGptWebTrace>,
    upstream_failure_kind: Option<String>,
    restricted_until: Option<String>,
    failover_class: Option<String>,
    terminal: TerminalRouteContext,
    agent_name: Option<String>,
    work_unit_id: Option<String>,
    work_unit_source: Option<crate::types::extensions::WorkUnitSource>,
    planned_hops: Option<u32>,
    plan_rebuilds: u32,
    route_memory_hit: Option<bool>,
    route_memory_invalidated: bool,
    source_model: Option<String>,
    json_schema_required: bool,
    replay: Option<crate::types::extensions::PlanReplaySnapshot>,
    repeat_429_violations: u32,
}

impl RouteTrace {
    pub(super) fn new_with_plan(
        candidates: usize,
        plan: Option<&crate::types::extensions::RoutePlanContext>,
    ) -> Self {
        Self {
            _started: Instant::now(),
            candidates,
            attempts: 0,
            skipped: 0,
            deepseek_web: None,
            chatgpt_web: None,
            upstream_failure_kind: None,
            restricted_until: None,
            failover_class: None,
            terminal: TerminalRouteContext::default(),
            agent_name: plan.map(|p| p.caller.agent_name.clone()),
            work_unit_id: plan.and_then(|p| p.caller.work_unit_id.clone()),
            work_unit_source: plan.map(|p| p.caller.work_unit_source),
            planned_hops: plan.map(|p| p.planned_hops),
            plan_rebuilds: 0,
            route_memory_hit: plan.map(|p| p.route_memory_hit),
            route_memory_invalidated: false,
            source_model: plan.and_then(|p| p.source_model.clone()),
            json_schema_required: plan.is_some_and(|p| p.json_schema_required),
            replay: plan.and_then(|p| p.replay.clone()),
            repeat_429_violations: 0,
        }
    }

    pub(super) fn set_replay(
        &mut self,
        replay: crate::types::extensions::PlanReplaySnapshot,
    ) {
        self.replay = Some(replay);
    }

    pub(super) fn set_plan_rebuilds(&mut self, count: u32) {
        self.plan_rebuilds = count;
    }

    pub(super) fn record_route_memory_invalidated(&mut self) {
        self.route_memory_invalidated = true;
    }

    pub(super) fn record_terminal(
        &mut self,
        limits: &ProviderLimitCatalog,
        candidate: &BudgetCandidate,
    ) {
        self.terminal = terminal_route_context(limits, candidate);
    }

    pub(super) fn record_deepseek_web(&mut self, trace: DeepSeekWebTrace) {
        self.deepseek_web = Some(trace);
    }

    pub(super) fn record_chatgpt_web(&mut self, trace: ChatGptWebTrace) {
        self.chatgpt_web = Some(trace);
    }

    pub(super) fn record_failure_signal(
        &mut self,
        class: crate::router::retry_after::FailoverClass,
        ctx: Option<&crate::types::extensions::UpstreamFailureContext>,
    ) {
        self.failover_class = Some(format!("{class:?}"));
        if let Some(ctx) = ctx {
            self.upstream_failure_kind = Some(format!("{:?}", ctx.kind));
            self.restricted_until =
                ctx.restricted_until.map(|dt| dt.to_rfc3339());
        }
    }

    pub(super) fn record_attempt(&mut self) {
        self.attempts = self.attempts.saturating_add(1);
    }

    pub(super) fn record_skipped(&mut self, count: usize) {
        self.skipped = self.skipped.saturating_add(count);
    }

    pub(super) fn record_repeat_429_violation(&mut self) {
        self.repeat_429_violations =
            self.repeat_429_violations.saturating_add(1);
    }

    pub(super) fn attempts(&self) -> u32 {
        self.attempts
    }

    pub(super) fn attach_pending(
        &self,
        router_id: &RouterId,
        strategy: &'static str,
        outcome: &RouteOutcome<'_>,
        intent_context: Option<crate::types::extensions::RoutingIntentContext>,
    ) -> PendingRouteTrace {
        PendingRouteTrace {
            router_id: router_id.clone(),
            strategy,
            hops: self.attempts,
            candidates: self.candidates,
            skipped: self.skipped,
            outcome_label: outcome.label,
            terminal_provider: outcome.provider.cloned(),
            terminal_credential: outcome
                .credential
                .map(ProviderCredentialId::to_string),
            terminal_status: outcome.status,
            deepseek_web: self.deepseek_web,
            chatgpt_web: self.chatgpt_web,
            intent_tier: intent_context.map(|c| c.intent_tier),
            selection_phase: intent_context.map(|c| c.selection_phase),
            quota_scope: self.terminal.quota_scope.clone(),
            model_ladder_band: self.terminal.model_ladder_band.clone(),
            model_ladder_position: self.terminal.model_ladder_position,
            upstream_failure_kind: self.upstream_failure_kind.clone(),
            restricted_until: self.restricted_until.clone(),
            failover_class: self.failover_class.clone(),
            agent_name: self.agent_name.clone(),
            work_unit_id: self.work_unit_id.clone(),
            work_unit_source: self.work_unit_source,
            planned_hops: self.planned_hops,
            plan_rebuilds: Some(self.plan_rebuilds),
            route_memory_hit: self.route_memory_hit,
            route_memory_invalidated: Some(self.route_memory_invalidated),
            source_model: self.source_model.clone(),
            json_schema_required: self.json_schema_required,
            replay: self.replay.clone(),
        }
    }

    pub(super) fn emit(
        &self,
        router_id: &RouterId,
        strategy: &'static str,
        outcome: &RouteOutcome<'_>,
        intent_context: Option<crate::types::extensions::RoutingIntentContext>,
    ) {
        crate::metrics::provider::emit_pending_route_trace(
            &self.attach_pending(router_id, strategy, outcome, intent_context),
            None,
            None,
        );
    }
}
