pub fn tier_cascade_kebab(
    c: crate::config::decision::TierCascade,
) -> &'static str {
    use crate::config::decision::TierCascade;

    match c {
        TierCascade::OnlyTier => "only-tier",
        TierCascade::PaidDown => "paid-down",
        TierCascade::FreeUp => "free-up",
    }
}
