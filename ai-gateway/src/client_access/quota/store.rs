use std::fmt;

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::config::client_access::ClientAccessWindowLimitsConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuotaFamily {
    Requests,
    Tokens,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuotaWindowKind {
    Minute,
    Day,
    Week,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QuotaDimension {
    pub family: QuotaFamily,
    pub window: QuotaWindowKind,
}

impl QuotaDimension {
    pub const REQUESTS_PER_MINUTE: Self = Self {
        family: QuotaFamily::Requests,
        window: QuotaWindowKind::Minute,
    };
    pub const REQUESTS_PER_DAY: Self = Self {
        family: QuotaFamily::Requests,
        window: QuotaWindowKind::Day,
    };
    pub const REQUESTS_PER_WEEK: Self = Self {
        family: QuotaFamily::Requests,
        window: QuotaWindowKind::Week,
    };
    pub const TOKENS_PER_MINUTE: Self = Self {
        family: QuotaFamily::Tokens,
        window: QuotaWindowKind::Minute,
    };
    pub const TOKENS_PER_DAY: Self = Self {
        family: QuotaFamily::Tokens,
        window: QuotaWindowKind::Day,
    };
    pub const TOKENS_PER_WEEK: Self = Self {
        family: QuotaFamily::Tokens,
        window: QuotaWindowKind::Week,
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("quota exceeded for {key_id}: {dimension:?}")]
pub struct QuotaRejection {
    pub key_id: String,
    pub dimension: QuotaDimension,
    pub limit: u64,
    pub used: u64,
    pub requested: u64,
    pub retry_after_seconds: u64,
}

impl QuotaRejection {
    #[must_use]
    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.used)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaLimitStatus {
    pub dimension: QuotaDimension,
    pub limit: u64,
    pub remaining: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuotaAdmission {
    pub most_constrained: Option<QuotaLimitStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaReservation {
    pub id: String,
    pub key_id: String,
    pub amount: u64,
    pub created_at: DateTime<Utc>,
    pub admission: QuotaAdmission,
}

#[derive(Debug, Error)]
pub enum QuotaAdmissionError {
    #[error("quota rejected")]
    Rejected(#[from] QuotaRejection),
    #[error("quota store failed: {0}")]
    Store(#[from] QuotaStoreError),
}

#[derive(Debug, Error)]
pub enum QuotaStoreError {
    #[error("quota store operation failed: {0}")]
    Operation(String),
    #[error(
        "quota reservation `{reservation_id}` not found for key `{key_id}`"
    )]
    ReservationNotFound {
        key_id: String,
        reservation_id: String,
    },
}

#[async_trait::async_trait]
pub trait ClientAccessQuotaStore: Send + Sync + fmt::Debug {
    async fn admit_request(
        &self,
        key_id: &str,
        limits: &ClientAccessWindowLimitsConfig,
        now: DateTime<Utc>,
    ) -> Result<QuotaAdmission, QuotaAdmissionError>;

    async fn reserve_tokens(
        &self,
        key_id: &str,
        amount: u64,
        limits: &ClientAccessWindowLimitsConfig,
        now: DateTime<Utc>,
    ) -> Result<QuotaReservation, QuotaAdmissionError>;

    async fn commit_tokens(
        &self,
        reservation: &QuotaReservation,
        actual_amount: u64,
        now: DateTime<Utc>,
    ) -> Result<(), QuotaStoreError>;

    async fn refund_tokens(
        &self,
        reservation: &QuotaReservation,
        now: DateTime<Utc>,
    ) -> Result<(), QuotaStoreError>;
}
