use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

const TPM_WINDOW: Duration = Duration::from_mins(1);

#[derive(Debug, Default)]
pub struct TpmWindow {
    entries: VecDeque<(Instant, u32)>,
}

impl TpmWindow {
    pub fn prune(&mut self, now: Instant) {
        while self
            .entries
            .front()
            .is_some_and(|(t, _)| now.duration_since(*t) >= TPM_WINDOW)
        {
            self.entries.pop_front();
        }
    }

    pub fn used(&mut self, now: Instant) -> u32 {
        self.prune(now);
        self.entries.iter().map(|(_, n)| n).sum()
    }

    pub fn would_exceed(
        &mut self,
        now: Instant,
        cap: u32,
        tokens: u32,
    ) -> bool {
        if tokens == 0 {
            return false;
        }
        self.used(now).saturating_add(tokens) > cap
    }

    pub fn record(&mut self, now: Instant, tokens: u32) {
        self.prune(now);
        if tokens > 0 {
            self.entries.push_back((now, tokens));
        }
    }

    pub fn wait_until_slot(
        &mut self,
        now: Instant,
        cap: u32,
        tokens: u32,
    ) -> Duration {
        if tokens == 0 || !self.would_exceed(now, cap, tokens) {
            return Duration::ZERO;
        }
        let Some((oldest, _)) = self.entries.front().copied() else {
            return Duration::ZERO;
        };
        TPM_WINDOW
            .saturating_sub(now.duration_since(oldest))
            .min(Duration::from_secs(5))
    }
}
