/// Cap upstream cooldown hints (`OmniRoute` `MAX_PROVIDER_COOLDOWN_MS`).
pub const MAX_COOLDOWN_SECS: u64 = 30 * 24 * 60 * 60;

#[must_use]
pub fn cap_duration_secs(secs: u64) -> u64 {
    secs.min(MAX_COOLDOWN_SECS)
}
