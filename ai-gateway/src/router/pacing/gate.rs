use std::{
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::{Mutex, Semaphore};

use super::{
    daily::{DailyQuotaWindow, DailyReject},
    limits::PacingLimits,
    tpm::TpmWindow,
    window::RpmWindow,
};

#[derive(Debug)]
struct GateState {
    rpm: RpmWindow,
    tpm: TpmWindow,
    daily: DailyQuotaWindow,
    last_start: Option<Instant>,
}

/// Composite gate: concurrent + RPM/TPM minute windows + RPD/TPD daily quota.
pub struct PacingGate {
    limits: PacingLimits,
    concurrent: Arc<Semaphore>,
    state: Mutex<GateState>,
}

impl PacingGate {
    #[must_use]
    pub fn new(limits: PacingLimits) -> Self {
        let daily_reset = limits.daily_reset_utc_hour;
        Self {
            concurrent: Arc::new(Semaphore::new(limits.concurrent)),
            limits,
            state: Mutex::new(GateState {
                rpm: RpmWindow::default(),
                tpm: TpmWindow::default(),
                daily: DailyQuotaWindow::new(daily_reset),
                last_start: None,
            }),
        }
    }

    /// Blocks until a slot is available or `max_queue_wait` elapses.
    pub async fn acquire(
        &self,
        estimated_tokens: u32,
    ) -> Result<PacingPermit, Duration> {
        let deadline = Instant::now() + self.limits.max_queue_wait;
        let permit = loop {
            let wait = self.next_wait(Instant::now(), estimated_tokens).await;
            if wait > Duration::ZERO {
                if Instant::now() + wait > deadline {
                    return Err(
                        deadline.saturating_duration_since(Instant::now())
                    );
                }
                tokio::time::sleep(
                    wait.min(
                        deadline.saturating_duration_since(Instant::now()),
                    ),
                )
                .await;
                continue;
            }
            self.check_daily(estimated_tokens).await?;
            match tokio::time::timeout_at(
                tokio::time::Instant::from_std(deadline),
                self.concurrent.clone().acquire_owned(),
            )
            .await
            {
                Ok(Ok(permit)) => break permit,
                Ok(Err(_)) => return Err(Duration::ZERO),
                Err(_) => {
                    return Err(
                        deadline.saturating_duration_since(Instant::now())
                    );
                }
            }
        };

        let mut state = self.state.lock().await;
        let now = Instant::now();
        state.daily.record(estimated_tokens);
        state.rpm.record(now);
        state.tpm.record(now, estimated_tokens);
        state.last_start = Some(now);

        Ok(PacingPermit { _permit: permit })
    }

    #[must_use]
    pub fn limits(&self) -> &PacingLimits {
        &self.limits
    }

    /// Read-only estimate of wait until the next pacing slot (no permit
    /// acquired).
    pub async fn peek_next_wait(&self, estimated_tokens: u32) -> Duration {
        self.next_wait(Instant::now(), estimated_tokens).await
    }

    /// Whether daily RPD/TPD quota still has headroom for `estimated_tokens`.
    pub async fn daily_headroom_available(
        &self,
        estimated_tokens: u32,
    ) -> bool {
        self.check_daily(estimated_tokens).await.is_ok()
    }

    async fn check_daily(&self, estimated_tokens: u32) -> Result<(), Duration> {
        let mut state = self.state.lock().await;
        match state.daily.would_reject(
            self.limits.rpd,
            self.limits.tpd,
            estimated_tokens,
        ) {
            Ok(()) => Ok(()),
            Err(DailyReject::Rpd | DailyReject::Tpd) => {
                Err(Duration::from_secs(state.daily.seconds_until_reset()))
            }
        }
    }

    async fn next_wait(&self, now: Instant, estimated_tokens: u32) -> Duration {
        let mut state = self.state.lock().await;
        let rpm_wait = if self.limits.has_rpm_limit() {
            state.rpm.wait_until_slot(now, self.limits.rpm)
        } else {
            Duration::ZERO
        };
        let tpm_wait = self.limits.tpm.map_or(Duration::ZERO, |cap| {
            state.tpm.wait_until_slot(now, cap, estimated_tokens)
        });
        let interval_wait = state.last_start.map_or(Duration::ZERO, |last| {
            self.limits
                .min_interval
                .saturating_sub(now.duration_since(last))
        });
        rpm_wait.max(tpm_wait).max(interval_wait)
    }
}

impl fmt::Debug for PacingGate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PacingGate")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

pub struct PacingPermit {
    _permit: tokio::sync::OwnedSemaphorePermit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn enforces_min_interval_between_starts() {
        let gate = PacingGate::new(PacingLimits {
            concurrent: 2,
            rpm: 60,
            tpm: None,
            rpd: None,
            tpd: None,
            daily_reset_utc_hour: 0,
            min_interval: Duration::from_millis(50),
            max_queue_wait: Duration::from_secs(2),
        });
        let _a = gate.acquire(0).await.unwrap();
        let start = Instant::now();
        let _b = gate.acquire(0).await.unwrap();
        assert!(start.elapsed() >= Duration::from_millis(45));
    }

    #[tokio::test]
    async fn rejects_when_queue_wait_exceeded() {
        let gate = PacingGate::new(PacingLimits {
            concurrent: 1,
            rpm: 60,
            tpm: None,
            rpd: None,
            tpd: None,
            daily_reset_utc_hour: 0,
            min_interval: Duration::from_secs(10),
            max_queue_wait: Duration::from_millis(80),
        });
        let _hold = gate.acquire(0).await.unwrap();
        assert!(gate.acquire(0).await.is_err());
    }

    #[tokio::test]
    async fn rejects_when_rpd_exhausted() {
        let gate = PacingGate::new(PacingLimits {
            concurrent: 4,
            rpm: u32::MAX,
            tpm: None,
            rpd: Some(1),
            tpd: None,
            daily_reset_utc_hour: 0,
            min_interval: Duration::ZERO,
            max_queue_wait: Duration::from_secs(1),
        });
        gate.acquire(0).await.unwrap();
        assert!(gate.acquire(0).await.is_err());
    }

    #[tokio::test]
    async fn rejects_when_tpm_exceeded() {
        let gate = PacingGate::new(PacingLimits {
            concurrent: 4,
            rpm: u32::MAX,
            tpm: Some(100),
            rpd: None,
            tpd: None,
            daily_reset_utc_hour: 0,
            min_interval: Duration::ZERO,
            max_queue_wait: Duration::from_millis(200),
        });
        gate.acquire(80).await.unwrap();
        assert!(gate.acquire(30).await.is_err());
    }

    #[tokio::test]
    async fn peek_next_wait_is_read_only() {
        let gate = PacingGate::new(PacingLimits {
            concurrent: 1,
            rpm: 60,
            tpm: None,
            rpd: None,
            tpd: None,
            daily_reset_utc_hour: 0,
            min_interval: Duration::from_millis(100),
            max_queue_wait: Duration::from_secs(1),
        });
        let _hold = gate.acquire(0).await.unwrap();
        let peek = gate.peek_next_wait(0).await;
        assert!(peek >= Duration::from_millis(90));
    }
}
