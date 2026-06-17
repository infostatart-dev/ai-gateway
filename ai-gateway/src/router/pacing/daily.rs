use chrono::{DateTime, Datelike, Duration as ChronoDuration, TimeZone, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DailyReject {
    Rpd,
    Tpd,
}

#[derive(Debug)]
pub struct DailyQuotaWindow {
    reset_utc_hour: u8,
    period_start: DateTime<Utc>,
    requests: u32,
    tokens: u32,
}

impl DailyQuotaWindow {
    #[must_use]
    pub fn new(reset_utc_hour: u8) -> Self {
        let now = Utc::now();
        Self {
            reset_utc_hour,
            period_start: period_start_at(now, reset_utc_hour),
            requests: 0,
            tokens: 0,
        }
    }

    pub fn roll(&mut self) {
        let now = Utc::now();
        let current = period_start_at(now, self.reset_utc_hour);
        if current != self.period_start {
            self.period_start = current;
            self.requests = 0;
            self.tokens = 0;
        }
    }

    pub fn would_reject(
        &mut self,
        rpd: Option<u32>,
        tpd: Option<u32>,
        tokens: u32,
    ) -> Result<(), DailyReject> {
        self.roll();
        if let Some(cap) = rpd
            && self.requests >= cap
        {
            return Err(DailyReject::Rpd);
        }
        if let Some(cap) = tpd
            && tokens > 0
            && self.tokens.saturating_add(tokens) > cap
        {
            return Err(DailyReject::Tpd);
        }
        Ok(())
    }

    pub fn record(&mut self, tokens: u32) {
        self.requests += 1;
        if tokens > 0 {
            self.tokens = self.tokens.saturating_add(tokens);
        }
    }

    #[must_use]
    pub fn seconds_until_reset(&self) -> u64 {
        seconds_until_reset(Utc::now(), self.reset_utc_hour)
    }
}

#[must_use]
pub fn period_start_at(now: DateTime<Utc>, reset_hour: u8) -> DateTime<Utc> {
    let h = u32::from(reset_hour);
    let today_reset = Utc
        .with_ymd_and_hms(now.year(), now.month(), now.day(), h, 0, 0)
        .single()
        .expect("valid reset hour");
    if now >= today_reset {
        today_reset
    } else {
        today_reset - ChronoDuration::days(1)
    }
}

#[must_use]
pub fn seconds_until_reset(now: DateTime<Utc>, reset_utc_hour: u8) -> u64 {
    let h = u32::from(reset_utc_hour);
    let today_reset = Utc
        .with_ymd_and_hms(now.year(), now.month(), now.day(), h, 0, 0)
        .single()
        .expect("valid reset hour");
    let next = if now >= today_reset {
        today_reset + ChronoDuration::days(1)
    } else {
        today_reset
    };
    u64::try_from((next - now).num_seconds().max(1)).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpd_rejects_at_limit() {
        let mut window = DailyQuotaWindow::new(0);
        window.would_reject(Some(2), None, 0).unwrap();
        window.record(0);
        window.would_reject(Some(2), None, 0).unwrap();
        window.record(0);
        assert_eq!(
            window.would_reject(Some(2), None, 0),
            Err(DailyReject::Rpd)
        );
    }

    #[test]
    fn tpd_rejects_when_tokens_exceed_cap() {
        let mut window = DailyQuotaWindow::new(0);
        window.would_reject(None, Some(100), 60).unwrap();
        window.record(60);
        assert_eq!(
            window.would_reject(None, Some(100), 50),
            Err(DailyReject::Tpd)
        );
    }
}
