use std::{
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::{Mutex, Semaphore};

use super::{limits::PacingLimits, window::RpmWindow};

#[derive(Debug)]
struct GateState {
    rpm: RpmWindow,
    last_start: Option<Instant>,
}

/// Composite gate: concurrent semaphore + RPM window + min interval (Template
/// Method steps).
pub struct PacingGate {
    limits: PacingLimits,
    concurrent: Arc<Semaphore>,
    state: Mutex<GateState>,
}

impl PacingGate {
    #[must_use]
    pub fn new(limits: PacingLimits) -> Self {
        Self {
            concurrent: Arc::new(Semaphore::new(limits.concurrent)),
            limits,
            state: Mutex::new(GateState {
                rpm: RpmWindow::default(),
                last_start: None,
            }),
        }
    }

    /// Blocks until a slot is available or `max_queue_wait` elapses.
    pub async fn acquire(&self) -> Result<PacingPermit, Duration> {
        let deadline = Instant::now() + self.limits.max_queue_wait;
        let permit =
            loop {
                let wait = self.next_wait(Instant::now()).await;
                if wait > Duration::ZERO {
                    if Instant::now() + wait > deadline {
                        return Err(
                            deadline.saturating_duration_since(Instant::now())
                        );
                    }
                    tokio::time::sleep(wait.min(
                        deadline.saturating_duration_since(Instant::now()),
                    ))
                    .await;
                    continue;
                }
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
        state.rpm.record(now);
        state.last_start = Some(now);

        Ok(PacingPermit { _permit: permit })
    }

    #[must_use]
    pub fn limits(&self) -> &PacingLimits {
        &self.limits
    }

    async fn next_wait(&self, now: Instant) -> Duration {
        let mut state = self.state.lock().await;
        let rpm_wait = state.rpm.wait_until_slot(now, self.limits.rpm);
        let interval_wait = state.last_start.map_or(Duration::ZERO, |last| {
            self.limits
                .min_interval
                .saturating_sub(now.duration_since(last))
        });
        rpm_wait.max(interval_wait)
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
            min_interval: Duration::from_millis(50),
            max_queue_wait: Duration::from_secs(2),
        });
        let _a = gate.acquire().await.unwrap();
        let start = Instant::now();
        let _b = gate.acquire().await.unwrap();
        assert!(start.elapsed() >= Duration::from_millis(45));
    }

    #[tokio::test]
    async fn rejects_when_queue_wait_exceeded() {
        let gate = PacingGate::new(PacingLimits {
            concurrent: 1,
            rpm: 60,
            min_interval: Duration::from_secs(10),
            max_queue_wait: Duration::from_millis(80),
        });
        let _hold = gate.acquire().await.unwrap();
        assert!(gate.acquire().await.is_err());
    }
}
