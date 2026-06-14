use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

const RPM_WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
pub struct RpmWindow {
    starts: VecDeque<Instant>,
}

impl RpmWindow {
    pub fn prune(&mut self, now: Instant) {
        while self
            .starts
            .front()
            .is_some_and(|t| now.duration_since(*t) >= RPM_WINDOW)
        {
            self.starts.pop_front();
        }
    }

    pub fn record(&mut self, now: Instant) {
        self.prune(now);
        self.starts.push_back(now);
    }

    pub fn wait_until_slot(&mut self, now: Instant, rpm: u32) -> Duration {
        self.prune(now);
        if self.starts.len() < rpm as usize {
            return Duration::ZERO;
        }
        let Some(oldest) = self.starts.front().copied() else {
            return Duration::ZERO;
        };
        RPM_WINDOW
            .saturating_sub(now.duration_since(oldest))
            .min(Duration::from_secs(5))
    }
}
