use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

use http::StatusCode;

use crate::{
    dispatcher::service::utils::extract_retry_after,
    types::{provider::InferenceProvider, response::Response},
};

pub const DEFAULT_PROVIDER_ERROR_COOLDOWN: Duration = Duration::from_secs(15);
pub const DEFAULT_RATE_LIMIT_COOLDOWN: Duration = Duration::from_mins(1);
pub const DEFAULT_AUTH_ERROR_COOLDOWN: Duration = Duration::from_mins(5);
pub const RETRY_AFTER_BUFFER: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Default)]
pub struct ProviderState {
    pub latency: Option<Duration>,
    pub cooldown_until: Option<Instant>,
    pub failures: u32,
}

pub fn lock_states(
    states: &Mutex<HashMap<InferenceProvider, ProviderState>>,
) -> MutexGuard<'_, HashMap<InferenceProvider, ProviderState>> {
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
            | StatusCode::REQUEST_TIMEOUT
            | StatusCode::CONFLICT
            | StatusCode::TOO_MANY_REQUESTS
    ) || status.is_server_error()
}

#[must_use]
pub fn cooldown_for_response(response: &Response) -> Duration {
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        return extract_retry_after(response.headers())
            .map_or(DEFAULT_RATE_LIMIT_COOLDOWN, Duration::from_secs)
            + RETRY_AFTER_BUFFER;
    }

    if matches!(
        response.status(),
        StatusCode::UNAUTHORIZED
            | StatusCode::FORBIDDEN
            | StatusCode::PAYMENT_REQUIRED
    ) {
        return DEFAULT_AUTH_ERROR_COOLDOWN;
    }

    DEFAULT_PROVIDER_ERROR_COOLDOWN
}
