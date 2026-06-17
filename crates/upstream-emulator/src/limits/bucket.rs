use std::time::{Duration, Instant};

use ai_gateway::config::provider_limits::{QuotaLimits, QuotaValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitVerdict {
    Allow,
    RpmExceeded,
    TpmExceeded,
    RpdExceeded,
    MinInterval,
    Concurrent,
}

#[derive(Debug, Clone)]
pub struct ApiLimits {
    pub rpm: u32,
    pub tpm: u32,
    pub rpd: Option<u32>,
    pub concurrent: usize,
    pub min_interval: Duration,
}

impl ApiLimits {
    pub fn from_quota(
        model: Option<&QuotaLimits>,
        tier: Option<&QuotaLimits>,
    ) -> Self {
        let source = model.or(tier).cloned().unwrap_or_default();
        Self {
            rpm: quota_u32(&source.rpm).unwrap_or(10_000),
            tpm: quota_u32(&source.tpm).unwrap_or(1_000_000),
            rpd: quota_u32(&source.rpd),
            concurrent: source
                .concurrent
                .and_then(|v| usize::try_from(v).ok())
                .unwrap_or(64),
            min_interval: source
                .min_interval_ms
                .map(Duration::from_millis)
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug)]
pub struct ScopeLimiter {
    limits: ApiLimits,
    minute_start: Instant,
    day_start: Instant,
    rpm_count: u32,
    tpm_count: u32,
    rpd_count: u32,
    in_flight: usize,
    last_start: Option<Instant>,
}

impl ScopeLimiter {
    #[must_use]
    pub fn new(limits: ApiLimits) -> Self {
        let now = Instant::now();
        Self {
            limits,
            minute_start: now,
            day_start: now,
            rpm_count: 0,
            tpm_count: 0,
            rpd_count: 0,
            in_flight: 0,
            last_start: None,
        }
    }

    #[must_use]
    pub fn rpm_used(&self) -> u32 {
        self.rpm_count
    }

    #[must_use]
    pub fn tpm_used(&self) -> u32 {
        self.tpm_count
    }

    #[must_use]
    pub fn rpd_used(&self) -> u32 {
        self.rpd_count
    }

    pub fn check_request(&mut self, tokens: u32) -> RateLimitVerdict {
        self.roll_windows();
        if self.limits.concurrent > 0
            && self.in_flight >= self.limits.concurrent
        {
            return RateLimitVerdict::Concurrent;
        }
        if let Some(last) = self.last_start
            && self.limits.min_interval > Duration::ZERO
            && last.elapsed() < self.limits.min_interval
        {
            return RateLimitVerdict::MinInterval;
        }
        if self.rpm_count >= self.limits.rpm {
            return RateLimitVerdict::RpmExceeded;
        }
        if self.tpm_count.saturating_add(tokens) > self.limits.tpm {
            return RateLimitVerdict::TpmExceeded;
        }
        if let Some(cap) = self.limits.rpd
            && self.rpd_count >= cap
        {
            return RateLimitVerdict::RpdExceeded;
        }
        self.rpm_count += 1;
        self.tpm_count = self.tpm_count.saturating_add(tokens);
        self.rpd_count += 1;
        self.in_flight += 1;
        self.last_start = Some(Instant::now());
        RateLimitVerdict::Allow
    }

    pub fn release(&mut self) {
        self.in_flight = self.in_flight.saturating_sub(1);
    }

    fn roll_windows(&mut self) {
        if self.minute_start.elapsed() >= Duration::from_mins(1) {
            self.minute_start = Instant::now();
            self.rpm_count = 0;
            self.tpm_count = 0;
        }
        if self.day_start.elapsed() >= Duration::from_hours(24) {
            self.day_start = Instant::now();
            self.rpd_count = 0;
        }
    }
}

fn quota_u32(value: &QuotaValue) -> Option<u32> {
    match value {
        QuotaValue::Limited(v) => u32::try_from(*v).ok(),
        QuotaValue::Unlimited | QuotaValue::Unknown => None,
    }
}
