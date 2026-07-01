use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
};

use chrono::{DateTime, Duration, Utc};

use super::{
    QuotaAdmission, QuotaAdmissionError, QuotaClock, QuotaDimension,
    QuotaFamily, QuotaLimitStatus, QuotaRejection, QuotaReservation,
    QuotaStoreError, QuotaWindow, QuotaWindowKind,
    store::ClientAccessQuotaStore,
};
use crate::config::client_access::ClientAccessWindowLimitsConfig;

#[derive(Debug)]
pub struct MemoryClientAccessQuotaStore {
    clock: QuotaClock,
    state: Mutex<MemoryQuotaState>,
}

impl MemoryClientAccessQuotaStore {
    #[must_use]
    pub fn new() -> Self {
        Self::with_clock(QuotaClock::default())
    }

    #[must_use]
    pub fn with_clock(clock: QuotaClock) -> Self {
        Self {
            clock,
            state: Mutex::new(MemoryQuotaState::default()),
        }
    }
}

impl Default for MemoryClientAccessQuotaStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ClientAccessQuotaStore for MemoryClientAccessQuotaStore {
    async fn admit_request(
        &self,
        key_id: &str,
        limits: &ClientAccessWindowLimitsConfig,
        now: DateTime<Utc>,
    ) -> Result<QuotaAdmission, QuotaAdmissionError> {
        let mut state = self.state.lock().expect("quota state lock poisoned");
        let key_state = state.key_mut(key_id);
        key_state.prepare(self.clock, now);
        let admission = check_all(
            key_id,
            QuotaFamily::Requests,
            limits,
            1,
            key_state,
            self.clock,
            now,
        )?;
        key_state.add(QuotaFamily::Requests, 1, now);
        Ok(admission)
    }

    async fn reserve_tokens(
        &self,
        key_id: &str,
        amount: u64,
        limits: &ClientAccessWindowLimitsConfig,
        now: DateTime<Utc>,
    ) -> Result<QuotaReservation, QuotaAdmissionError> {
        let amount = i64::try_from(amount).unwrap_or(i64::MAX);
        let mut state = self.state.lock().expect("quota state lock poisoned");
        let reservation_id = state.next_reservation_id();
        let key_state = state.key_mut(key_id);
        key_state.prepare(self.clock, now);
        let admission = check_all(
            key_id,
            QuotaFamily::Tokens,
            limits,
            u64::try_from(amount).unwrap_or(u64::MAX),
            key_state,
            self.clock,
            now,
        )?;
        key_state.add(QuotaFamily::Tokens, amount, now);
        key_state
            .reservations
            .insert(reservation_id.clone(), amount);
        Ok(QuotaReservation {
            id: reservation_id,
            key_id: key_id.to_string(),
            amount: u64::try_from(amount).unwrap_or(u64::MAX),
            created_at: now,
            admission,
        })
    }

    async fn commit_tokens(
        &self,
        reservation: &QuotaReservation,
        actual_amount: u64,
        now: DateTime<Utc>,
    ) -> Result<(), QuotaStoreError> {
        let actual_amount = i64::try_from(actual_amount).unwrap_or(i64::MAX);
        let mut state = self
            .state
            .lock()
            .map_err(|_| QuotaStoreError::Operation("lock poisoned".into()))?;
        let Some(key_state) = state.keys.get_mut(&reservation.key_id) else {
            return Err(QuotaStoreError::ReservationNotFound {
                key_id: reservation.key_id.clone(),
                reservation_id: reservation.id.clone(),
            });
        };
        let Some(reserved_amount) =
            key_state.reservations.remove(&reservation.id)
        else {
            return Err(QuotaStoreError::ReservationNotFound {
                key_id: reservation.key_id.clone(),
                reservation_id: reservation.id.clone(),
            });
        };
        let delta = actual_amount.saturating_sub(reserved_amount);
        if delta != 0 {
            key_state.prepare(self.clock, now);
            key_state.add(QuotaFamily::Tokens, delta, now);
        }
        Ok(())
    }

    async fn refund_tokens(
        &self,
        reservation: &QuotaReservation,
        now: DateTime<Utc>,
    ) -> Result<(), QuotaStoreError> {
        self.commit_tokens(reservation, 0, now).await
    }
}

#[derive(Debug, Default)]
struct MemoryQuotaState {
    next_reservation: u64,
    keys: HashMap<String, KeyQuotaState>,
}

impl MemoryQuotaState {
    fn next_reservation_id(&mut self) -> String {
        self.next_reservation = self.next_reservation.saturating_add(1);
        format!("mem-{}", self.next_reservation)
    }

    fn key_mut(&mut self, key_id: &str) -> &mut KeyQuotaState {
        self.keys.entry(key_id.to_string()).or_default()
    }
}

#[derive(Debug, Default)]
struct KeyQuotaState {
    requests_minute: VecDeque<QuotaEvent>,
    tokens_minute: VecDeque<QuotaEvent>,
    requests_day: BucketCounter,
    requests_week: BucketCounter,
    tokens_day: BucketCounter,
    tokens_week: BucketCounter,
    reservations: HashMap<String, i64>,
}

impl KeyQuotaState {
    fn prepare(&mut self, clock: QuotaClock, now: DateTime<Utc>) {
        prune_rolling(&mut self.requests_minute, now);
        prune_rolling(&mut self.tokens_minute, now);
        self.requests_day.ensure_window(clock.day(now));
        self.tokens_day.ensure_window(clock.day(now));
        self.requests_week.ensure_window(clock.iso_week(now));
        self.tokens_week.ensure_window(clock.iso_week(now));
    }

    fn add(&mut self, family: QuotaFamily, amount: i64, now: DateTime<Utc>) {
        match family {
            QuotaFamily::Requests => {
                self.requests_minute
                    .push_back(QuotaEvent { at: now, amount });
                self.requests_day.add(amount);
                self.requests_week.add(amount);
            }
            QuotaFamily::Tokens => {
                self.tokens_minute.push_back(QuotaEvent { at: now, amount });
                self.tokens_day.add(amount);
                self.tokens_week.add(amount);
            }
        }
    }

    fn used(&self, dimension: QuotaDimension) -> u64 {
        match dimension {
            QuotaDimension {
                family: QuotaFamily::Requests,
                window: QuotaWindowKind::Minute,
            } => rolling_used(&self.requests_minute),
            QuotaDimension {
                family: QuotaFamily::Requests,
                window: QuotaWindowKind::Day,
            } => self.requests_day.used(),
            QuotaDimension {
                family: QuotaFamily::Requests,
                window: QuotaWindowKind::Week,
            } => self.requests_week.used(),
            QuotaDimension {
                family: QuotaFamily::Tokens,
                window: QuotaWindowKind::Minute,
            } => rolling_used(&self.tokens_minute),
            QuotaDimension {
                family: QuotaFamily::Tokens,
                window: QuotaWindowKind::Day,
            } => self.tokens_day.used(),
            QuotaDimension {
                family: QuotaFamily::Tokens,
                window: QuotaWindowKind::Week,
            } => self.tokens_week.used(),
        }
    }

    fn retry_after(
        &self,
        dimension: QuotaDimension,
        clock: QuotaClock,
        now: DateTime<Utc>,
    ) -> u64 {
        match dimension.window {
            QuotaWindowKind::Minute => {
                let events = match dimension.family {
                    QuotaFamily::Requests => &self.requests_minute,
                    QuotaFamily::Tokens => &self.tokens_minute,
                };
                events.front().map_or(1, |event| {
                    let reset_at = event.at + Duration::seconds(60);
                    u64::try_from((reset_at - now).num_seconds().max(1))
                        .unwrap_or(u64::MAX)
                })
            }
            QuotaWindowKind::Day => clock.day(now).retry_after_seconds(now),
            QuotaWindowKind::Week => {
                clock.iso_week(now).retry_after_seconds(now)
            }
        }
    }
}

#[derive(Debug, Default)]
struct BucketCounter {
    window: Option<QuotaWindow>,
    used: i64,
}

impl BucketCounter {
    fn ensure_window(&mut self, window: QuotaWindow) {
        if self.window != Some(window) {
            self.window = Some(window);
            self.used = 0;
        }
    }

    fn add(&mut self, amount: i64) {
        self.used = self.used.saturating_add(amount).max(0);
    }

    fn used(&self) -> u64 {
        u64::try_from(self.used.max(0)).unwrap_or(u64::MAX)
    }
}

#[derive(Debug, Clone, Copy)]
struct QuotaEvent {
    at: DateTime<Utc>,
    amount: i64,
}

fn prune_rolling(events: &mut VecDeque<QuotaEvent>, now: DateTime<Utc>) {
    let cutoff = now - Duration::seconds(60);
    while events.front().is_some_and(|event| event.at <= cutoff) {
        events.pop_front();
    }
}

fn rolling_used(events: &VecDeque<QuotaEvent>) -> u64 {
    let used = events
        .iter()
        .fold(0_i64, |sum, event| sum.saturating_add(event.amount))
        .max(0);
    u64::try_from(used).unwrap_or(u64::MAX)
}

fn check_all(
    key_id: &str,
    family: QuotaFamily,
    limits: &ClientAccessWindowLimitsConfig,
    requested: u64,
    key_state: &KeyQuotaState,
    clock: QuotaClock,
    now: DateTime<Utc>,
) -> Result<QuotaAdmission, QuotaRejection> {
    let mut most_constrained = None;
    for (window, limit) in [
        (QuotaWindowKind::Minute, limits.per_minute),
        (QuotaWindowKind::Day, limits.per_day),
        (QuotaWindowKind::Week, limits.per_week),
    ] {
        let Some(limit) = limit else {
            continue;
        };
        let dimension = QuotaDimension { family, window };
        let used = key_state.used(dimension);
        if used.saturating_add(requested) > limit {
            return Err(QuotaRejection {
                key_id: key_id.to_string(),
                dimension,
                limit,
                used,
                requested,
                retry_after_seconds: key_state
                    .retry_after(dimension, clock, now),
            });
        }
        let remaining = limit.saturating_sub(used.saturating_add(requested));
        let status = QuotaLimitStatus {
            dimension,
            limit,
            remaining,
        };
        if most_constrained
            .as_ref()
            .is_none_or(|current: &QuotaLimitStatus| {
                status.remaining < current.remaining
            })
        {
            most_constrained = Some(status);
        }
    }
    Ok(QuotaAdmission { most_constrained })
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[tokio::test]
    async fn request_minute_limit_is_per_key() {
        let store = MemoryClientAccessQuotaStore::new();
        let limits = ClientAccessWindowLimitsConfig {
            per_minute: Some(1),
            per_day: None,
            per_week: None,
        };
        let now = Utc.with_ymd_and_hms(2026, 6, 29, 12, 0, 0).unwrap();

        store.admit_request("key-a", &limits, now).await.unwrap();
        assert!(store.admit_request("key-a", &limits, now).await.is_err());
        assert!(store.admit_request("key-b", &limits, now).await.is_ok());
    }

    #[tokio::test]
    async fn request_rejection_reports_dimension_metadata() {
        let store = MemoryClientAccessQuotaStore::new();
        let limits = ClientAccessWindowLimitsConfig {
            per_minute: Some(1),
            per_day: None,
            per_week: None,
        };
        let now = Utc.with_ymd_and_hms(2026, 6, 29, 12, 0, 0).unwrap();

        store.admit_request("key-a", &limits, now).await.unwrap();
        let error = store
            .admit_request("key-a", &limits, now)
            .await
            .unwrap_err();

        let QuotaAdmissionError::Rejected(rejection) = error else {
            panic!("expected quota rejection");
        };
        assert_eq!(rejection.key_id, "key-a");
        assert_eq!(rejection.dimension, QuotaDimension::REQUESTS_PER_MINUTE);
        assert_eq!(rejection.limit, 1);
        assert_eq!(rejection.used, 1);
        assert_eq!(rejection.requested, 1);
        assert!(rejection.retry_after_seconds > 0);
    }

    #[tokio::test]
    async fn request_day_and_week_windows_reset_independently() {
        let store = MemoryClientAccessQuotaStore::new();
        let limits = ClientAccessWindowLimitsConfig {
            per_minute: None,
            per_day: Some(1),
            per_week: Some(2),
        };
        let monday = Utc.with_ymd_and_hms(2026, 6, 29, 12, 0, 0).unwrap();
        let tuesday = Utc.with_ymd_and_hms(2026, 6, 30, 12, 0, 0).unwrap();
        let next_monday = Utc.with_ymd_and_hms(2026, 7, 6, 12, 0, 0).unwrap();

        store.admit_request("key-a", &limits, monday).await.unwrap();
        assert!(store.admit_request("key-a", &limits, monday).await.is_err());
        store
            .admit_request("key-a", &limits, tuesday)
            .await
            .unwrap();
        assert!(
            store
                .admit_request("key-a", &limits, tuesday)
                .await
                .is_err()
        );
        assert!(
            store
                .admit_request("key-a", &limits, next_monday)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn token_reservation_refunds_unused_amount() {
        let store = MemoryClientAccessQuotaStore::new();
        let limits = ClientAccessWindowLimitsConfig {
            per_minute: Some(100),
            per_day: Some(100),
            per_week: Some(100),
        };
        let now = Utc.with_ymd_and_hms(2026, 6, 29, 12, 0, 0).unwrap();

        let reservation = store
            .reserve_tokens("key-a", 80, &limits, now)
            .await
            .unwrap();
        store.commit_tokens(&reservation, 40, now).await.unwrap();

        assert!(
            store
                .reserve_tokens("key-a", 60, &limits, now)
                .await
                .is_ok()
        );
        assert!(
            store
                .reserve_tokens("key-a", 1, &limits, now)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn token_commit_records_reported_overage_as_debt() {
        let store = MemoryClientAccessQuotaStore::new();
        let limits = ClientAccessWindowLimitsConfig {
            per_minute: Some(130),
            per_day: Some(130),
            per_week: Some(130),
        };
        let now = Utc.with_ymd_and_hms(2026, 6, 29, 12, 0, 0).unwrap();

        let reservation = store
            .reserve_tokens("key-a", 80, &limits, now)
            .await
            .unwrap();
        store.commit_tokens(&reservation, 130, now).await.unwrap();

        assert!(
            store
                .reserve_tokens("key-a", 1, &limits, now)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn token_estimated_commit_keeps_reserved_amount() {
        let store = MemoryClientAccessQuotaStore::new();
        let limits = ClientAccessWindowLimitsConfig {
            per_minute: Some(100),
            per_day: Some(100),
            per_week: Some(100),
        };
        let now = Utc.with_ymd_and_hms(2026, 6, 29, 12, 0, 0).unwrap();

        let reservation = store
            .reserve_tokens("key-a", 80, &limits, now)
            .await
            .unwrap();
        store
            .commit_tokens(&reservation, reservation.amount, now)
            .await
            .unwrap();

        assert!(
            store
                .reserve_tokens("key-a", 20, &limits, now)
                .await
                .is_ok()
        );
        assert!(
            store
                .reserve_tokens("key-a", 1, &limits, now)
                .await
                .is_err()
        );
    }
}
