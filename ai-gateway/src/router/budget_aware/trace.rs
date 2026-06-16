//! Per-request routing trace summary emitted once at the end of failover so ops
//! can see hops, duration and the terminal outcome without grepping every hop.

use std::time::Instant;

use crate::{
    config::credentials::ProviderCredentialId,
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
}

impl RouteTrace {
    pub(super) fn new(candidates: usize) -> Self {
        Self {
            _started: Instant::now(),
            candidates,
            attempts: 0,
            skipped: 0,
            deepseek_web: None,
        }
    }

    pub(super) fn record_deepseek_web(&mut self, trace: DeepSeekWebTrace) {
        self.deepseek_web = Some(trace);
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
        }
    }

    pub(super) fn emit(
        &self,
        router_id: &RouterId,
        strategy: &'static str,
        outcome: &RouteOutcome<'_>,
    ) {
        crate::metrics::provider::emit_pending_route_trace(
            &self.attach_pending(router_id, strategy, outcome),
            None,
            None,
        );
    }
}
