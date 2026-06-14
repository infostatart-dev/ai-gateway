use super::constants::cap_duration_secs;

#[must_use]
pub fn parse_retry_delay_seconds(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(ms) = raw.strip_suffix("ms").or_else(|| raw.strip_suffix("MS")) {
        let ms: u64 = ms.trim().parse().ok()?;
        return Some(cap_duration_secs(ms.div_ceil(1000).max(1)));
    }
    if let Some(seconds) = raw.strip_suffix('s').or_else(|| raw.strip_suffix('S')) {
        return parse_decimal_seconds(seconds).map(cap_duration_secs);
    }
    if let Some(hours) = raw.strip_suffix('h').or_else(|| raw.strip_suffix('H')) {
        let hours: u64 = hours.trim().parse().ok()?;
        return Some(cap_duration_secs(hours.saturating_mul(3600)));
    }
    if let Some(minutes) = raw.strip_suffix('m').or_else(|| raw.strip_suffix('M')) {
        let minutes: u64 = minutes.trim().parse().ok()?;
        return Some(cap_duration_secs(minutes.saturating_mul(60)));
    }
    raw.parse::<u64>()
        .ok()
        .map(cap_duration_secs)
}

#[must_use]
pub fn parse_try_again_in_seconds(text: &str) -> Option<u64> {
    let lower = text.to_ascii_lowercase();
    let marker = "try again in ";
    let start = lower.find(marker)? + marker.len();
    let rest = &text[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    parse_decimal_seconds(&rest[..end]).map(cap_duration_secs)
}

#[must_use]
pub fn parse_decimal_seconds(raw: &str) -> Option<u64> {
    let value: f64 = raw.trim().parse().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value.ceil() as u64)
}

#[must_use]
pub fn parse_hms_groups_seconds(raw: &str) -> Option<u64> {
    let mut total = 0u64;
    let mut chunk = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            chunk.push(ch);
            continue;
        }
        let unit = ch.to_ascii_lowercase();
        if unit == 'h' {
            let hours: u64 = chunk.parse().ok()?;
            total = total.saturating_add(hours.saturating_mul(3600));
            chunk.clear();
        } else if unit == 'm' {
            let minutes: u64 = chunk.parse().ok()?;
            total = total.saturating_add(minutes.saturating_mul(60));
            chunk.clear();
        } else if unit == 's' {
            total = total.saturating_add(parse_decimal_seconds(&chunk)?);
            chunk.clear();
        }
    }
    (total > 0).then_some(cap_duration_secs(total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_google_retry_delay_suffix() {
        assert_eq!(parse_retry_delay_seconds("15.002899939s"), Some(16));
    }

    #[test]
    fn parses_groq_try_again_message() {
        let text = "Rate limit reached... Please try again in 54.918s. Need more tokens?";
        assert_eq!(parse_try_again_in_seconds(text), Some(55));
    }
}
