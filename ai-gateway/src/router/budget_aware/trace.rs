//! Per-request routing trace summary emitted once at the end of failover so ops
//! can see hops, duration and the terminal outcome without grepping every hop.

use std::time::Instant;

use crate::{
    config::credentials::ProviderCredentialId,
    types::{provider::InferenceProvider, router::RouterId},
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
    started: Instant,
    candidates: usize,
    attempts: u32,
    skipped: usize,
    deepseek_web: Option<DeepSeekWebTrace>,
}

impl RouteTrace {
    pub(super) fn new(candidates: usize) -> Self {
        Self {
            started: Instant::now(),
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
            deepseek_web_turns = self.deepseek_web.map_or(0, |d| d.turns),
            deepseek_web_upload_parts =
                self.deepseek_web.map_or(0, |d| d.upload_parts),
            deepseek_web_pow_cache_hits =
                self.deepseek_web.map_or(0, |d| d.pow_cache_hits),
            "budget-aware route summary"
        );
    }
}
