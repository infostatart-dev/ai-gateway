use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaWindow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl QuotaWindow {
    #[must_use]
    pub fn retry_after_seconds(&self, now: DateTime<Utc>) -> u64 {
        let seconds = (self.end - now).num_seconds().max(0);
        u64::try_from(seconds).unwrap_or(u64::MAX)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QuotaClock {
    day_reset_hour_utc: u32,
}

impl QuotaClock {
    #[must_use]
    pub fn new(day_reset_hour_utc: u32) -> Self {
        Self {
            day_reset_hour_utc: day_reset_hour_utc.min(23),
        }
    }

    #[must_use]
    pub fn rolling_minute(&self, now: DateTime<Utc>) -> QuotaWindow {
        QuotaWindow {
            start: now - Duration::seconds(60),
            end: now,
        }
    }

    #[must_use]
    pub fn day(&self, now: DateTime<Utc>) -> QuotaWindow {
        let today_reset = Utc
            .with_ymd_and_hms(
                now.year(),
                now.month(),
                now.day(),
                self.day_reset_hour_utc,
                0,
                0,
            )
            .single()
            .expect("valid UTC day reset");
        let start = if now.hour() < self.day_reset_hour_utc {
            today_reset - Duration::days(1)
        } else {
            today_reset
        };
        QuotaWindow {
            start,
            end: start + Duration::days(1),
        }
    }

    #[must_use]
    pub fn iso_week(&self, now: DateTime<Utc>) -> QuotaWindow {
        let date = now.date_naive();
        let monday = date
            - Duration::days(i64::from(date.weekday().num_days_from_monday()));
        let start = Utc.from_utc_datetime(
            &monday
                .and_hms_opt(0, 0, 0)
                .expect("midnight is valid for all dates"),
        );
        QuotaWindow {
            start,
            end: start + Duration::weeks(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_window_respects_reset_hour() {
        let clock = QuotaClock::new(3);
        let now = Utc.with_ymd_and_hms(2026, 6, 29, 2, 0, 0).unwrap();
        let window = clock.day(now);
        assert_eq!(window.start.day(), 28);
        assert_eq!(window.end.day(), 29);
    }

    #[test]
    fn iso_week_starts_on_monday() {
        let clock = QuotaClock::default();
        let now = Utc.with_ymd_and_hms(2026, 7, 2, 12, 0, 0).unwrap();
        let window = clock.iso_week(now);
        assert_eq!(window.start.weekday(), chrono::Weekday::Mon);
        assert_eq!(window.end - window.start, Duration::weeks(1));
    }
}
