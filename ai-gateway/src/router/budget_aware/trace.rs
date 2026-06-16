//! Per-request routing trace summary emitted once at the end of failover so ops
//! can see hops, duration and the terminal outcome without grepping every hop.

use std::time::Instant;

use crate::{
    config::credentials::ProviderCredentialId,
    types::{provider::InferenceProvider, router::RouterId},
};

/// The terminal upstream a request settled on (or none).
pub(super) struct RouteOutcome<'a> {
    pub label: &'static str,
    pub provider: Option<&'a InferenceProvider>,
    pub credential: Option<&'a ProviderCredentialId>,
    pub status: Option<u16>,
}

pub(super) struct RouteTrace {
    started: Instant,
    candidates: usize,
    attempts: u32,
    skipped: usize,
}

impl RouteTrace {
    pub(super) fn new(candidates: usize) -> Self {
        Self {
            started: Instant::now(),
            candidates,
            attempts: 0,
            skipped: 0,
        }
    }

    pub(super) fn record_attempt(&mut self) {
        self.attempts = self.attempts.saturating_add(1);
    }

    pub(super) fn record_skipped(&mut self, count: usize) {
        self.skipped = self.skipped.saturating_add(count);
    }

    pub(super) fn emit(
        &self,
        router_id: &RouterId,
        strategy: &'static str,
        outcome: &RouteOutcome<'_>,
    ) {
        let provider = outcome
            .provider
            .map_or_else(|| "none".to_string(), ToString::to_string);
        let credential = outcome
            .credential
            .map_or("none", ProviderCredentialId::as_str);
        let duration_ms = u64::try_from(self.started.elapsed().as_millis())
            .unwrap_or(u64::MAX);
        tracing::info!(
            router_id = %router_id,
            strategy,
            outcome = outcome.label,
            hops = self.attempts,
            candidates = self.candidates,
            skipped = self.skipped,
            duration_ms,
            terminal_provider = provider,
            terminal_credential = credential,
            terminal_status = outcome.status.map_or(0, u32::from),
            "budget-aware route summary"
        );
    }
}
