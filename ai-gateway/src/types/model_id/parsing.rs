use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};

use super::version::Version;

pub(crate) fn parse_date(input: &str) -> Option<(DateTime<Utc>, &'static str)> {
    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d")
        && let Some(naive_dt) = date.and_hms_opt(0, 0, 0)
    {
        return Some((Utc.from_utc_datetime(&naive_dt), "%Y-%m-%d"));
    }
    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y%m%d")
        && let Some(naive_dt) = date.and_hms_opt(0, 0, 0)
    {
        return Some((Utc.from_utc_datetime(&naive_dt), "%Y%m%d"));
    }
    if let Ok(date) = NaiveDate::parse_from_str(
        &format!("{}-{}", Utc::now().year(), input),
        "%Y-%m-%d",
    ) && let Some(naive_dt) = date.and_hms_opt(0, 0, 0)
    {
        return Some((Utc.from_utc_datetime(&naive_dt), "%m-%d"));
    }
    if let Ok(date) = NaiveDate::parse_from_str(
        &format!("{}{}", Utc::now().year(), input),
        "%Y%m%d",
    ) && let Some(naive_dt) = date.and_hms_opt(0, 0, 0)
    {
        return Some((Utc.from_utc_datetime(&naive_dt), "%m%d"));
    }
    None
}

pub(crate) fn parse_model_and_version(
    s: &str,
    separator: char,
) -> (&str, Option<Version>) {
    if let Some(preview_pos) = s.rfind("-preview-") {
        let after_preview = &s[preview_pos + 9..];
        if let Some((dt, fmt)) = parse_date(after_preview) {
            return (
                &s[..preview_pos],
                Some(Version::DateVersionedPreview {
                    date: dt,
                    format: fmt,
                }),
            );
        }
    }
    if let Some(model) = s.strip_suffix("-preview") {
        return (model, Some(Version::Preview));
    }
    if let Some(model) = s.strip_suffix("-latest") {
        return (model, Some(Version::Latest));
    }

    let mut candidates = Vec::new();
    for (idx, ch) in s.char_indices().rev() {
        if ch == separator && idx != s.len() - 1 {
            candidates.push((idx, &s[idx + 1..]));
        }
    }
    candidates.reverse();

    for (idx, candidate) in &candidates {
        if let Some((dt, fmt)) = parse_date(candidate)
            && (fmt == "%Y-%m-%d" || fmt == "%Y%m%d")
        {
            return (
                &s[..*idx],
                Some(Version::Date {
                    date: dt,
                    format: fmt,
                }),
            );
        }
    }
    for (idx, candidate) in &candidates {
        if let Some((dt, fmt)) = parse_date(candidate) {
            return (
                &s[..*idx],
                Some(Version::Date {
                    date: dt,
                    format: fmt,
                }),
            );
        }
        if candidate.eq_ignore_ascii_case("latest") {
            return (&s[..*idx], Some(Version::Latest));
        }
        if candidate.eq_ignore_ascii_case("preview") {
            return (&s[..*idx], Some(Version::Preview));
        }
    }
    (s, None)
}
