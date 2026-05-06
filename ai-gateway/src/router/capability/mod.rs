use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::{Duration, Instant},
};

use axum_core::body::Body;
use bytes::Bytes;
use futures::future::BoxFuture;
use http_body_util::BodyExt;
use nonempty_collections::NESet;
use tower::{Service, ServiceExt};

use crate::{
    app_state::AppState,
    config::{
        decision::{DecisionTier, TierCascade},
        router::RouterConfig,
    },
    dispatcher::{Dispatcher, DispatcherService},
    error::{api::ApiError, internal::InternalError},
    middleware::{
        decision::policy::{KeyPolicy, Tier},
        mapper::model::ModelMapper,
    },
    router::provider_attempt::{
        ProviderState, cooldown_for_response, is_failoverable_status,
        lock_states, smoothed_latency,
    },
    types::{
        model_id::{ModelId, ModelIdWithoutVersion},
        provider::InferenceProvider,
        request::Request,
        response::Response,
        router::RouterId,
    },
};

mod providers;

#[cfg(test)]
mod tests;

/// Tier chain for cascade mode from `start`. Used for tier-aware candidate
/// ordering (same ordering idea as `cascade_chain` in the traffic shaper).
fn tier_chain_for_models(start: Tier, cascade: TierCascade) -> Vec<Tier> {
    match cascade {
        TierCascade::OnlyTier => vec![start],
        TierCascade::PaidDown => {
            let order = [Tier::Paid, Tier::Freemium, Tier::Free];
            tier_slice_from(start, &order)
        }
        TierCascade::FreeUp => {
            let order = [Tier::Free, Tier::Freemium, Tier::Paid];
            tier_slice_from(start, &order)
        }
    }
}

fn tier_slice_from(start: Tier, order: &[Tier]) -> Vec<Tier> {
    if let Some(idx) = order.iter().position(|t| *t == start) {
        order[idx..].to_vec()
    } else {
        vec![start]
    }
}

#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct ModelCapability {
    pub provider: InferenceProvider,
    pub model: ModelId,
    pub context_window: Option<u32>,
    pub supports_tools: bool,
    pub supports_json_schema: bool,
    pub supports_vision: bool,
    pub reasoning: bool,
}

#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct RequestRequirements {
    pub min_context_tokens: Option<u32>,
    pub tools_required: bool,
    pub json_schema_required: bool,
    pub vision_required: bool,
    pub reasoning_preferred: bool,
}

pub fn extract_requirements(req_body: &Bytes) -> RequestRequirements {
    let Ok(value): Result<serde_json::Value, _> =
        serde_json::from_slice(req_body)
    else {
        return RequestRequirements::default();
    };

    let tools_required = value
        .get("tools")
        .and_then(|v| v.as_array())
        .is_some_and(|a| !a.is_empty());

    let json_schema_required = value
        .get("response_format")
        .and_then(|v| v.get("type"))
        .and_then(|v| v.as_str())
        == Some("json_schema");

    let vision_required = value
        .get("messages")
        .and_then(|v| v.as_array())
        .is_some_and(|messages| {
            messages.iter().any(|m| {
                m.get("content").and_then(|c| c.as_array()).is_some_and(
                    |contents| {
                        contents.iter().any(|c| {
                            c.get("type").and_then(|t| t.as_str())
                                == Some("image_url")
                        })
                    },
                )
            })
        });

    let model_name = value.get("model").and_then(|v| v.as_str()).unwrap_or("");
    let reasoning_preferred = ["o1", "o3", "o4", "reasoner", "thinking"]
        .iter()
        .any(|&keyword| model_name.contains(keyword));

    RequestRequirements {
        min_context_tokens: None,
        tools_required,
        json_schema_required,
        vision_required,
        reasoning_preferred,
    }
}

pub(crate) fn extract_source_model(req_body: &Bytes) -> Option<ModelId> {
    let value: serde_json::Value = serde_json::from_slice(req_body).ok()?;
    let model = value.get("model").and_then(|v| v.as_str())?;

    ModelId::from_str(model)
        .or_else(|_| {
            ModelId::from_str_and_provider(InferenceProvider::OpenAI, model)
        })
        .ok()
}

pub fn supports(
    requirements: &RequestRequirements,
    model: &ModelCapability,
) -> bool {
    if requirements.tools_required && !model.supports_tools {
        return false;
    }
    if requirements.json_schema_required && !model.supports_json_schema {
        return false;
    }
    if requirements.vision_required && !model.supports_vision {
        return false;
    }
    if let Some(min) = requirements.min_context_tokens {
        match model.context_window {
            Some(window) if window >= min => {}
            _ => return false,
        }
    }
    true
}

pub(crate) fn get_model_capability(
    provider: &InferenceProvider,
    model: &ModelId,
) -> ModelCapability {
    let model_name = model.to_string().to_lowercase();

    let mut cap = ModelCapability {
        provider: provider.clone(),
        model: model.clone(),
        context_window: None,
        supports_tools: false,
        supports_json_schema: false,
        supports_vision: false,
        reasoning: ["o1", "o3", "o4", "reasoner", "thinking"]
            .iter()
            .any(|&keyword| model_name.contains(keyword)),
    };

    providers::apply_provider_capabilities(&mut cap, provider, &model_name);
    cap
}

#[derive(Debug, Clone)]
struct CapabilityCandidate {
    capability: ModelCapability,
    service: DispatcherService,
    /// Tier from `decision.model-tiers` at router build time.
    /// `None`: model not listed (tier filter skips it for bucketing).
    tier: Option<DecisionTier>,
}

#[derive(Debug, Clone)]
pub struct CapabilityAwareRouter {
    candidates: Arc<Vec<CapabilityCandidate>>,
    model_mapper: ModelMapper,
    states: Arc<Mutex<HashMap<InferenceProvider, ProviderState>>>,
    default_latency: Duration,
    cascade: TierCascade,
    tiers_configured: bool,
}

impl CapabilityAwareRouter {
    pub async fn new(
        app_state: AppState,
        router_id: RouterId,
        router_config: Arc<RouterConfig>,
        providers: &NESet<InferenceProvider>,
    ) -> Result<Self, crate::error::init::InitError> {
        let mut candidates = Vec::new();
        let providers_config = &app_state.config().providers;
        let model_tiers = &app_state.config().decision.model_tiers;

        for provider in providers {
            if let Some(config) = providers_config.get(provider) {
                for model in &config.models {
                    let capability = get_model_capability(provider, model);

                    let service = Dispatcher::new_with_model_id_without_rate_limit_events(
                        app_state.clone(),
                        &router_id,
                        &router_config,
                        provider.clone(),
                        model.clone(),
                    )
                    .await?;

                    let tier = model_tiers.tier_of(model);

                    candidates.push(CapabilityCandidate {
                        capability,
                        service,
                        tier,
                    });
                }
            }
        }

        Ok(Self {
            candidates: Arc::new(candidates),
            model_mapper: ModelMapper::new_for_router(
                app_state.clone(),
                router_config.clone(),
            ),
            states: Arc::new(Mutex::new(HashMap::new())),
            default_latency: app_state.config().discover.default_rtt,
            cascade: router_config
                .decision
                .tier_cascade
                .unwrap_or(app_state.config().decision.shaper.cascade),
            tiers_configured: !model_tiers.is_empty(),
        })
    }

    fn rank_candidates(
        &self,
        candidates: &mut [CapabilityCandidate],
        requirements: &RequestRequirements,
    ) {
        let now = Instant::now();
        let states = lock_states(&self.states);

        candidates.sort_by(|a, b| {
            let state_a = states.get(&a.capability.provider);
            let state_b = states.get(&b.capability.provider);

            let cooling_a = state_a
                .is_some_and(|s| s.cooldown_until.is_some_and(|u| u > now));
            let cooling_b = state_b
                .is_some_and(|s| s.cooldown_until.is_some_and(|u| u > now));

            cooling_a
                .cmp(&cooling_b)
                .then_with(|| {
                    let failures_a = state_a.map_or(0, |s| s.failures);
                    let failures_b = state_b.map_or(0, |s| s.failures);
                    failures_a.cmp(&failures_b)
                })
                .then_with(|| {
                    let lat_a = state_a
                        .and_then(|s| s.latency)
                        .unwrap_or(self.default_latency);
                    let lat_b = state_b
                        .and_then(|s| s.latency)
                        .unwrap_or(self.default_latency);
                    lat_a.cmp(&lat_b)
                })
                // Reasoning preferred ranking
                .then_with(|| {
                    let r_a = a.capability.reasoning
                        == requirements.reasoning_preferred;
                    let r_b = b.capability.reasoning
                        == requirements.reasoning_preferred;
                    r_b.cmp(&r_a) // true (matched preference) should come first
                })
                .then_with(|| {
                    a.capability
                        .model
                        .to_string()
                        .cmp(&b.capability.model.to_string())
                })
        });
    }

    fn matches_source_model(
        &self,
        source_model: &ModelId,
        candidate: &CapabilityCandidate,
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

    fn ordered_candidates(
        &self,
        requirements: &RequestRequirements,
        source_model: Option<&ModelId>,
        policy_tier: Option<Tier>,
    ) -> Result<Vec<CapabilityCandidate>, InternalError> {
        let mut filtered: Vec<_> = self
            .candidates
            .iter()
            .filter(|c| {
                supports(requirements, &c.capability)
                    && source_model.is_none_or(|source_model| {
                        self.matches_source_model(source_model, c)
                    })
            })
            .cloned()
            .collect();

        if filtered.is_empty() {
            let has_hard_requirements = requirements.tools_required
                || requirements.json_schema_required
                || requirements.vision_required
                || requirements.min_context_tokens.is_some();

            if has_hard_requirements || source_model.is_some() {
                tracing::warn!(
                    ?requirements,
                    ?source_model,
                    "No candidates match request model or hard requirements"
                );
                return Err(InternalError::ProviderNotFound);
            }

            // Fallback only if no hard requirements
            let mut all = self.candidates.as_ref().clone();
            self.rank_candidates(&mut all, requirements);
            return Ok(self.apply_tier_cascade(all, policy_tier));
        }

        self.rank_candidates(&mut filtered, requirements);
        Ok(self.apply_tier_cascade(filtered, policy_tier))
    }

    /// Reorders candidates by tier cascade: start tier first, then cascade
    /// order, then unclassified models (tail). No-op if `model_tiers` is
    /// empty or `policy_tier` is missing.
    fn apply_tier_cascade(
        &self,
        candidates: Vec<CapabilityCandidate>,
        policy_tier: Option<Tier>,
    ) -> Vec<CapabilityCandidate> {
        if !self.tiers_configured {
            return candidates;
        }
        let Some(start_tier) = policy_tier else {
            return candidates;
        };
        let chain = tier_chain_for_models(start_tier, self.cascade);

        let mut buckets: Vec<Vec<CapabilityCandidate>> =
            vec![Vec::new(); chain.len()];
        let mut tail: Vec<CapabilityCandidate> = Vec::new();

        for cand in candidates {
            let cand_tier = cand.tier.map(Tier::from);
            match cand_tier {
                Some(tier) => {
                    if let Some(idx) = chain.iter().position(|t| *t == tier) {
                        buckets[idx].push(cand);
                    } else {
                        // Tier set but outside current cascade chain — append
                        // to tail.
                        tail.push(cand);
                    }
                }
                None => tail.push(cand),
            }
        }

        let mut ordered: Vec<CapabilityCandidate> =
            buckets.into_iter().flatten().collect();
        ordered.extend(tail);
        ordered
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

impl Service<Request> for CapabilityAwareRouter {
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
            let policy_tier =
                req.extensions().get::<KeyPolicy>().map(|p| p.tier);
            let (parts, body) = req.into_parts();
            let body_bytes = body
                .collect()
                .await
                .map_err(InternalError::CollectBodyError)?
                .to_bytes();

            let requirements = extract_requirements(&body_bytes);
            let source_model = extract_source_model(&body_bytes);
            let candidates = this.ordered_candidates(
                &requirements,
                source_model.as_ref(),
                policy_tier,
            )?;
            let mut failed_providers = HashSet::new();

            for candidate in &candidates {
                if failed_providers.contains(&candidate.capability.provider) {
                    continue;
                }

                let req = Request::from_parts(
                    parts.clone(),
                    Body::from(body_bytes.clone()),
                );
                let start = Instant::now();
                let service = candidate.service.clone();
                let response = service.oneshot(req).await.map_err(|_| {
                    ApiError::Internal(InternalError::ProviderNotFound)
                })?;

                let elapsed = start.elapsed();
                let status = response.status();

                if is_failoverable_status(status) {
                    this.record_failure(
                        &candidate.capability.provider,
                        &response,
                        elapsed,
                    );
                    failed_providers
                        .insert(candidate.capability.provider.clone());
                    continue;
                }

                this.record_success(&candidate.capability.provider, elapsed);
                return Ok(response);
            }

            Err(ApiError::Internal(InternalError::ProviderNotFound))
        })
    }
}
