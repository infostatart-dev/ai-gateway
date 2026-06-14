use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

use http::StatusCode;

use crate::{
    config::credentials::ProviderCredentialId,
    types::provider::InferenceProvider,
};

#[derive(Debug, Clone, Default)]
pub struct ProviderState {
    pub latency: Option<Duration>,
    pub cooldown_until: Option<Instant>,
    pub failures: u32,
}

pub fn lock_provider_states(
    states: &Mutex<HashMap<InferenceProvider, ProviderState>>,
) -> MutexGuard<'_, HashMap<InferenceProvider, ProviderState>> {
    states
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn lock_credential_states(
    states: &Mutex<HashMap<ProviderCredentialId, ProviderState>>,
) -> MutexGuard<'_, HashMap<ProviderCredentialId, ProviderState>> {
    states
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[must_use]
pub fn smoothed_latency(
    current: Option<Duration>,
    observed: Duration,
) -> Duration {
    current.map_or(observed, |current| {
        let current = current.as_micros();
        let observed = observed.as_micros();
        let smoothed =
            (current.saturating_mul(7) + observed.saturating_mul(3)) / 10;
        Duration::from_micros(smoothed.try_into().unwrap_or(u64::MAX))
    })
}

#[must_use]
pub fn is_failoverable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::BAD_REQUEST
            | StatusCode::PAYMENT_REQUIRED
            | StatusCode::UNAUTHORIZED
            | StatusCode::FORBIDDEN
            | StatusCode::NOT_FOUND
            | StatusCode::PAYLOAD_TOO_LARGE
            | StatusCode::REQUEST_TIMEOUT
            | StatusCode::CONFLICT
            | StatusCode::TOO_MANY_REQUESTS
    ) || status.is_server_error()
}
