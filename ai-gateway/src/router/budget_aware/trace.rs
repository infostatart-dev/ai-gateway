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
}

impl RouteTrace {
    pub(super) fn new(candidates: usize) -> Self {
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
        }
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
