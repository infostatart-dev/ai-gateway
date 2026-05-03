use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::{Duration, Instant},
};

use axum_core::body::Body;
use futures::future::BoxFuture;
use http_body_util::BodyExt;
use indexmap::IndexMap;
use nonempty_collections::NESet;
use tower::{Service, ServiceExt};

use crate::{
    app_state::AppState,
    config::router::RouterConfig,
    dispatcher::{Dispatcher, DispatcherService},
    error::{api::ApiError, init::InitError, internal::InternalError},
    middleware::mapper::model::ModelMapper,
    router::{
        capability::{
            ModelCapability, RequestRequirements, extract_requirements,
            extract_source_model, get_model_capability, supports,
        },
        provider_attempt::{
            ProviderState, cooldown_for_response, is_failoverable_status,
            lock_states, smoothed_latency,
        },
    },
    types::{
        model_id::{ModelId, ModelIdWithoutVersion},
        provider::InferenceProvider,
        request::Request,
        response::Response,
        router::RouterId,
    },
};

#[derive(Debug, Clone)]
struct BudgetCandidate {
    capability: ModelCapability,
    service: DispatcherService,
}

#[derive(Debug, Clone)]
pub struct BudgetAwareRouter {
    candidates: Arc<Vec<BudgetCandidate>>,
    model_mapper: ModelMapper,
    states: Arc<Mutex<HashMap<InferenceProvider, ProviderState>>>,
    provider_priorities: Arc<IndexMap<InferenceProvider, u16>>,
    default_latency: Duration,
    max_cooldown_wait: Duration,
}

impl BudgetAwareRouter {
    pub async fn new(
        app_state: AppState,
        router_id: RouterId,
        router_config: Arc<RouterConfig>,
        providers: &NESet<InferenceProvider>,
        provider_priorities: &IndexMap<InferenceProvider, u16>,
        max_cooldown_wait: Duration,
    ) -> Result<Self, InitError> {
        let mut candidates = Vec::new();
        let providers_config = &app_state.config().providers;

        for provider in providers {
            if let Some(config) = providers_config.get(provider) {
                for model in &config.models {
                    let capability = get_model_capability(provider, model);
                    let service =
                        Dispatcher::new_with_model_id_without_rate_limit_events(
                            app_state.clone(),
                            &router_id,
                            &router_config,
                            provider.clone(),
                            model.clone(),
                        )
                        .await?;

                    candidates.push(BudgetCandidate {
                        capability,
                        service,
                    });
                }
            }
        }

        Ok(Self {
            candidates: Arc::new(candidates),
            model_mapper: ModelMapper::new_for_router(
                app_state.clone(),
                router_config,
            ),
            states: Arc::new(Mutex::new(HashMap::new())),
            provider_priorities: Arc::new(provider_priorities.clone()),
            default_latency: app_state.config().discover.default_rtt,
            max_cooldown_wait,
        })
    }

    fn ordered_candidates(
        &self,
        requirements: &RequestRequirements,
        source_model: Option<&ModelId>,
    ) -> Result<Vec<BudgetCandidate>, InternalError> {
        let mut candidates = self
            .candidates
            .iter()
            .filter(|candidate| {
                supports(requirements, &candidate.capability)
                    && source_model.is_none_or(|source_model| {
                        self.matches_source_model(source_model, candidate)
                    })
            })
            .cloned()
            .collect::<Vec<_>>();

        if candidates.is_empty() {
            tracing::warn!(
                ?requirements,
                ?source_model,
                "no budget-aware candidate matched request"
            );
            return Err(InternalError::ProviderNotFound);
        }

        self.rank_candidates(&mut candidates, requirements);
        Ok(candidates)
    }

    fn matches_source_model(
        &self,
        source_model: &ModelId,
        candidate: &BudgetCandidate,
    ) -> bool {
        self.model_mapper
            .map_model(source_model, &candidate.capability.provider)
            .is_ok_and(|target_model| {
                ModelIdWithoutVersion::from(target_model)
                    == ModelIdWithoutVersion::from(
                        candidate.capability.model.clone(),
                    )
            })
    }

    fn rank_candidates(
        &self,
        candidates: &mut [BudgetCandidate],
        requirements: &RequestRequirements,
    ) {
        let now = Instant::now();
        let states = lock_states(&self.states);

        candidates.sort_by(|left, right| {
            let left_state = states.get(&left.capability.provider);
            let right_state = states.get(&right.capability.provider);

            self.effective_budget_rank(left, left_state, now)
                .cmp(&self.effective_budget_rank(right, right_state, now))
                .then_with(|| {
                    let left_reasoning = left.capability.reasoning
                        == requirements.reasoning_preferred;
                    let right_reasoning = right.capability.reasoning
                        == requirements.reasoning_preferred;
                    right_reasoning.cmp(&left_reasoning)
                })
                .then_with(|| {
                    let left_failures = left_state.map_or(0, |s| s.failures);
                    let right_failures = right_state.map_or(0, |s| s.failures);
                    left_failures.cmp(&right_failures)
                })
                .then_with(|| {
                    let left_latency = left_state
                        .and_then(|s| s.latency)
                        .unwrap_or(self.default_latency);
                    let right_latency = right_state
                        .and_then(|s| s.latency)
                        .unwrap_or(self.default_latency);
                    left_latency.cmp(&right_latency)
                })
                .then_with(|| {
                    left.capability
                        .model
                        .to_string()
                        .cmp(&right.capability.model.to_string())
                })
        });
    }

    fn effective_budget_rank(
        &self,
        candidate: &BudgetCandidate,
        state: Option<&ProviderState>,
        now: Instant,
    ) -> u16 {
        let base = self.budget_rank(candidate);
        let remaining_cooldown = state
            .and_then(|state| state.cooldown_until)
            .and_then(|until| until.checked_duration_since(now));

        effective_budget_rank(base, remaining_cooldown, self.max_cooldown_wait)
    }

    fn budget_rank(&self, candidate: &BudgetCandidate) -> u16 {
        self.provider_priorities
            .get(&candidate.capability.provider)
            .copied()
            .unwrap_or_else(|| default_budget_rank(&candidate.capability))
    }

    fn record_success(&self, provider: &InferenceProvider, elapsed: Duration) {
        let mut states = lock_states(&self.states);
        let state = states.entry(provider.clone()).or_default();
        state.latency = Some(smoothed_latency(state.latency, elapsed));
        state.cooldown_until = None;
        state.failures = 0;
    }

    fn record_failure(
        &self,
        provider: &InferenceProvider,
        response: &Response,
        elapsed: Duration,
    ) {
        let mut states = lock_states(&self.states);
        let state = states.entry(provider.clone()).or_default();
        state.latency = Some(smoothed_latency(state.latency, elapsed));
        state.failures = state.failures.saturating_add(1);
        state.cooldown_until =
            Some(Instant::now() + cooldown_for_response(response));
    }
}

impl Service<Request> for BudgetAwareRouter {
    type Response = Response;
    type Error = ApiError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let this = self.clone();
        Box::pin(async move {
            let (parts, body) = req.into_parts();
            let body_bytes = body
                .collect()
                .await
                .map_err(InternalError::CollectBodyError)?
                .to_bytes();
            let requirements = extract_requirements(&body_bytes);
            let source_model = extract_source_model(&body_bytes);
            let candidates =
                this.ordered_candidates(&requirements, source_model.as_ref())?;
            let mut failed_providers = HashSet::new();

            for (index, candidate) in candidates.iter().enumerate() {
                if failed_providers.contains(&candidate.capability.provider) {
                    continue;
                }

                let has_next_provider =
                    candidates[index + 1..].iter().any(|next| {
                        next.capability.provider
                            != candidate.capability.provider
                            && !failed_providers
                                .contains(&next.capability.provider)
                    });
                if !this.wait_for_candidate(candidate, has_next_provider).await
                {
                    continue;
                }

                let req = Request::from_parts(
                    parts.clone(),
                    Body::from(body_bytes.clone()),
                );
                let start = Instant::now();
                let response = call_candidate(candidate, req).await?;
                let elapsed = start.elapsed();
                let status = response.status();

                if has_next_provider && is_failoverable_status(status) {
                    this.record_failure(
                        &candidate.capability.provider,
                        &response,
                        elapsed,
                    );
                    failed_providers
                        .insert(candidate.capability.provider.clone());
                    tracing::warn!(
                        provider = %candidate.capability.provider,
                        model = %candidate.capability.model,
                        status = %status,
                        "budget-aware router failed over to next candidate"
                    );
                    continue;
                }

                if status.is_success() {
                    this.record_success(
                        &candidate.capability.provider,
                        elapsed,
                    );
                } else if is_failoverable_status(status) {
                    this.record_failure(
                        &candidate.capability.provider,
                        &response,
                        elapsed,
                    );
                }
                return Ok(response);
            }

            Err(ApiError::Internal(InternalError::ProviderNotFound))
        })
    }
}

impl BudgetAwareRouter {
    async fn wait_for_candidate(
        &self,
        candidate: &BudgetCandidate,
        has_next_provider: bool,
    ) -> bool {
        let remaining = {
            let states = lock_states(&self.states);
            states
                .get(&candidate.capability.provider)
                .and_then(|state| state.cooldown_until)
                .and_then(|until| until.checked_duration_since(Instant::now()))
        };

        let Some(remaining) = remaining else {
            return true;
        };
        if remaining <= self.max_cooldown_wait {
            tracing::debug!(
                provider = %candidate.capability.provider,
                model = %candidate.capability.model,
                wait_ms = remaining.as_millis(),
                "waiting for cheap budget-aware candidate cooldown"
            );
            tokio::time::sleep(remaining).await;
            return true;
        }

        if has_next_provider {
            tracing::debug!(
                provider = %candidate.capability.provider,
                model = %candidate.capability.model,
                cooldown_ms = remaining.as_millis(),
                "skipping candidate with cooldown above budget wait"
            );
            return false;
        }

        tokio::time::sleep(self.max_cooldown_wait).await;
        true
    }
}

async fn call_candidate(
    candidate: &BudgetCandidate,
    req: Request,
) -> Result<Response, ApiError> {
    candidate
        .service
        .clone()
        .oneshot(req)
        .await
        .map_err(infallible_to_api_error)
}

fn infallible_to_api_error(error: Infallible) -> ApiError {
    match error {}
}

fn effective_budget_rank(
    base_rank: u16,
    remaining_cooldown: Option<Duration>,
    max_cooldown_wait: Duration,
) -> u16 {
    base_rank
        .saturating_mul(10)
        .saturating_add(remaining_cooldown.map_or(0, |remaining| {
            if remaining <= max_cooldown_wait {
                5
            } else {
                1_000
            }
        }))
}

fn default_budget_rank(capability: &ModelCapability) -> u16 {
    match &capability.provider {
        InferenceProvider::Ollama => 0,
        InferenceProvider::Named(name) if name == "groq" => 0,
        InferenceProvider::GoogleGemini => 1,
        InferenceProvider::Named(name) if name == "deepseek" => 10,
        InferenceProvider::OpenRouter
            if capability.model.to_string().ends_with(":free") =>
        {
            0
        }
        InferenceProvider::OpenRouter => 20,
        InferenceProvider::OpenAI => 30,
        InferenceProvider::Anthropic => 40,
        InferenceProvider::Bedrock => 50,
        InferenceProvider::Named(_) => 25,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn capability(provider: InferenceProvider, model: &str) -> ModelCapability {
        let model =
            ModelId::from_str_and_provider(provider.clone(), model).unwrap();
        ModelCapability {
            provider,
            model,
            context_window: None,
            supports_tools: false,
            supports_json_schema: false,
            supports_vision: false,
            reasoning: false,
        }
    }

    #[test]
    fn ranks_short_cooldown_cheap_provider_before_expensive_provider() {
        let groq = capability(
            InferenceProvider::Named("groq".into()),
            "llama-3.1-8b-instant",
        );
        let anthropic =
            capability(InferenceProvider::Anthropic, "claude-3-7-sonnet");

        assert!(
            effective_budget_rank(
                default_budget_rank(&groq),
                Some(Duration::from_secs(2)),
                Duration::from_secs(3),
            ) < effective_budget_rank(
                default_budget_rank(&anthropic),
                None,
                Duration::from_secs(3),
            )
        );
    }
}
