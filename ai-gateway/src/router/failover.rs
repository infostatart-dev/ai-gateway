use std::{
    collections::HashMap,
    convert::Infallible,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::{Duration, Instant},
};

use futures::future::BoxFuture;
use http_body_util::BodyExt;
use nonempty_collections::NESet;
use tower::{Service, ServiceExt};

use crate::{
    app_state::AppState,
    config::router::RouterConfig,
    dispatcher::{Dispatcher, DispatcherService},
    error::{api::ApiError, internal::InternalError},
    router::provider_attempt::{
        ProviderState, cooldown_for_response, is_failoverable_status,
        lock_states, smoothed_latency,
    },
    types::{
        provider::InferenceProvider, request::Request, response::Response,
        router::RouterId,
    },
};

#[derive(Debug, Clone)]
struct ProviderCandidate {
    provider: InferenceProvider,
    service: DispatcherService,
}

#[derive(Debug, Clone)]
pub struct ProviderFailoverRouter {
    candidates: Arc<Vec<ProviderCandidate>>,
    states: Arc<Mutex<HashMap<InferenceProvider, ProviderState>>>,
    default_latency: Duration,
}

impl ProviderFailoverRouter {
    pub async fn new(
        app_state: AppState,
        router_id: RouterId,
        router_config: Arc<RouterConfig>,
        providers: &NESet<InferenceProvider>,
    ) -> Result<Self, crate::error::init::InitError> {
        let mut providers = providers.iter().cloned().collect::<Vec<_>>();
        providers.sort_by_key(ToString::to_string);

        let mut candidates = Vec::with_capacity(providers.len());
        for provider in providers {
            let service = Dispatcher::new_without_rate_limit_events(
                app_state.clone(),
                &router_id,
                &router_config,
                provider.clone(),
            )
            .await?;
            candidates.push(ProviderCandidate { provider, service });
        }

        Ok(Self {
            candidates: Arc::new(candidates),
            states: Arc::new(Mutex::new(HashMap::new())),
            default_latency: app_state.config().discover.default_rtt,
        })
    }

    fn ordered_candidates(&self) -> Vec<ProviderCandidate> {
        let now = Instant::now();
        let states = lock_states(&self.states);
        let mut candidates = self
            .candidates
            .iter()
            .map(|candidate| {
                let state = states.get(&candidate.provider);
                let latency = state
                    .and_then(|state| state.latency)
                    .unwrap_or(self.default_latency);
                let failures = state.map_or(0, |state| state.failures);
                let cooldown_until =
                    state.and_then(|state| state.cooldown_until);
                (
                    cooldown_until.is_some_and(|until| until > now),
                    cooldown_until.unwrap_or(now),
                    failures,
                    latency,
                    candidate.provider.to_string(),
                    candidate.clone(),
                )
            })
            .collect::<Vec<_>>();

        candidates.sort_by(
            |(
                left_cooling_down,
                left_cooldown_until,
                left_failures,
                left_latency,
                left_provider,
                _,
            ),
             (
                right_cooling_down,
                right_cooldown_until,
                right_failures,
                right_latency,
                right_provider,
                _,
            )| {
                left_cooling_down
                    .cmp(right_cooling_down)
                    .then_with(|| left_cooldown_until.cmp(right_cooldown_until))
                    .then_with(|| left_failures.cmp(right_failures))
                    .then_with(|| left_latency.cmp(right_latency))
                    .then_with(|| left_provider.cmp(right_provider))
            },
        );

        candidates
            .into_iter()
            .map(|(_, _, _, _, _, candidate)| candidate)
            .collect()
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

impl Service<Request> for ProviderFailoverRouter {
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
            let candidates = this.ordered_candidates();
            let (parts, body) = req.into_parts();
            let body = body
                .collect()
                .await
                .map_err(InternalError::CollectBodyError)?
                .to_bytes();

            for (index, candidate) in candidates.iter().enumerate() {
                let req = Request::from_parts(
                    parts.clone(),
                    axum_core::body::Body::from(body.clone()),
                );
                let start = Instant::now();
                let response = call_candidate(candidate, req).await?;
                let elapsed = start.elapsed();
                let status = response.status();
                let has_next = index + 1 < candidates.len();

                if is_failoverable_status(status) {
                    this.record_failure(
                        &candidate.provider,
                        &response,
                        elapsed,
                    );
                    tracing::warn!(
                        provider = %candidate.provider,
                        status = %status,
                        has_next = has_next,
                        "provider failed, trying next candidate"
                    );
                    continue;
                }

                this.record_success(&candidate.provider, elapsed);
                return Ok(response);
            }

            Err(ApiError::Internal(InternalError::ProviderNotFound))
        })
    }
}

async fn call_candidate(
    candidate: &ProviderCandidate,
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
