use std::fmt::Write as _;

use crate::{
    config::{
        Config,
        decision::{DecisionTier, TierCascade},
        router::RouterConfig,
    },
    middleware::decision::policy::Tier,
};

pub fn write_decision_status(
    out: &mut String,
    config: &Config,
    router_config: &RouterConfig,
) {
    let (engine_label, color) = if router_config.decision.enabled {
        ("enabled", "\x1b[32m")
    } else {
        ("disabled", "\x1b[90m")
    };
    writeln!(
        out,
        "  Decision engine         : {color}{engine_label}\x1b[0m"
    )
    .expect("write to String");

    if !router_config.decision.enabled {
        return;
    }

    let start_tier = Tier::from(config.decision.default_policy.tier);
    let (cascade, cascade_source) =
        resolved_tier_cascade(config, router_config.decision);
    let chain = cascade_chain(start_tier, cascade);

    writeln!(
        out,
        "  Default policy tier     : \x1b[33m{}\x1b[0m (starting slot pool \
         for requests without a per-key policy)",
        tier_label(config.decision.default_policy.tier)
    )
    .expect("write to String");

    let (mode, mode_hint) = cascade_mode(cascade);
    writeln!(
        out,
        "  Tier cascade            : \x1b[33m{mode}\x1b[0m ({cascade_source}) \
         — {mode_hint}"
    )
    .expect("write to String");

    writeln!(
        out,
        "  Fallback when slots full: \x1b[33m{}\x1b[0m",
        format_cascade_chain(&chain)
    )
    .expect("write to String");

    writeln!(
        out,
        "  Per-request tier header : X-Decision-Tier: free | freemium | paid"
    )
    .expect("write to String");
}

fn resolved_tier_cascade(
    config: &Config,
    router_decision: crate::config::decision::RouterDecisionConfig,
) -> (TierCascade, &'static str) {
    if let Some(cascade) = router_decision.tier_cascade {
        (cascade, "this router")
    } else {
        (config.decision.shaper.cascade, "global config")
    }
}

fn tier_label(tier: DecisionTier) -> &'static str {
    match tier {
        DecisionTier::Free => "free",
        DecisionTier::Freemium => "freemium",
        DecisionTier::Paid => "paid",
    }
}

fn cascade_mode(cascade: TierCascade) -> (&'static str, &'static str) {
    match cascade {
        TierCascade::OnlyTier => (
            "only-tier",
            "no escalation; reject when the start tier is full",
        ),
        TierCascade::PaidDown => {
            ("paid-down", "try cheaper tiers when the start tier is full")
        }
        TierCascade::FreeUp => (
            "free-up",
            "try more expensive tiers when the start tier is full",
        ),
    }
}

fn cascade_chain(start: Tier, cascade: TierCascade) -> Vec<Tier> {
    match cascade {
        TierCascade::OnlyTier => vec![start],
        TierCascade::PaidDown => {
            slice_from(start, &[Tier::Paid, Tier::Freemium, Tier::Free])
        }
        TierCascade::FreeUp => {
            slice_from(start, &[Tier::Free, Tier::Freemium, Tier::Paid])
        }
    }
}

fn slice_from(start: Tier, order: &[Tier]) -> Vec<Tier> {
    if let Some(idx) = order.iter().position(|tier| *tier == start) {
        order[idx..].to_vec()
    } else {
        vec![start]
    }
}

fn format_cascade_chain(chain: &[Tier]) -> String {
    if chain.len() <= 1 {
        return format!("{} only (no cascade)", runtime_tier_label(chain[0]));
    }
    chain
        .iter()
        .map(|tier| runtime_tier_label(*tier))
        .collect::<Vec<_>>()
        .join(" → ")
}

fn runtime_tier_label(tier: Tier) -> &'static str {
    match tier {
        Tier::Free => "free",
        Tier::Freemium => "freemium",
        Tier::Paid => "paid",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::decision::RouterDecisionConfig;

    #[test]
    fn freemium_free_up_chain_is_freemium_then_paid() {
        let chain = cascade_chain(Tier::Freemium, TierCascade::FreeUp);
        assert_eq!(format_cascade_chain(&chain), "freemium → paid");
    }

    #[test]
    fn free_free_up_chain_includes_all_tiers() {
        let chain = cascade_chain(Tier::Free, TierCascade::FreeUp);
        assert_eq!(format_cascade_chain(&chain), "free → freemium → paid");
    }

    #[test]
    fn only_tier_shows_single_tier() {
        let chain = cascade_chain(Tier::Freemium, TierCascade::OnlyTier);
        assert_eq!(format_cascade_chain(&chain), "freemium only (no cascade)");
    }

    #[test]
    fn write_decision_status_includes_policy_tier_and_cascade() {
        use crate::tests::TestDefault;

        let config = Config::test_default();
        let router = RouterConfig {
            decision: RouterDecisionConfig {
                enabled: true,
                tier_cascade: Some(TierCascade::FreeUp),
            },
            ..Default::default()
        };
        let mut out = String::new();
        write_decision_status(&mut out, &config, &router);
        assert!(out.contains("Default policy tier"));
        assert!(out.contains("freemium"));
        assert!(out.contains("Tier cascade"));
        assert!(out.contains("free-up"));
        assert!(out.contains("freemium → paid"));
    }
}
