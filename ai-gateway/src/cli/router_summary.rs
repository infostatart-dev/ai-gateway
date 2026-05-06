use std::fmt::Write as _;

use crate::{
    cli::{decision_display, provider_order},
    config::Config,
    endpoints::EndpointType,
};

pub fn print_configured_router_sections(config: &Config) {
    for (router_id, router_config) in config.routers.as_ref() {
        if router_id == &Config::autodefault_router_id() {
            continue;
        }
        if !router_config
            .load_balance
            .as_ref()
            .contains_key(&EndpointType::Chat)
        {
            continue;
        }

        let mut section = format!(
            "\n\n\x1b[1mRouter {router_id}\x1b[0m  \
             /router/{router_id}/chat/completions\n",
        );
        print_decision_status(&mut section, router_config);
        print_strategy_and_providers(&mut section, router_config);
        print!("{section}");
    }
}

pub fn print_decision_status(
    out: &mut String,
    router_config: &crate::config::router::RouterConfig,
) {
    let (label, color) = if router_config.decision.enabled {
        ("enabled", "\x1b[32m")
    } else {
        ("disabled", "\x1b[90m")
    };
    writeln!(out, "  Decision engine : {color}{label}\x1b[0m")
        .expect("write to String");

    if router_config.decision.enabled {
        let cascade = router_config
            .decision
            .tier_cascade
            .map_or("global-default", decision_display::tier_cascade_kebab);
        writeln!(
            out,
            "  Tier cascade (this router) : \x1b[33m{cascade}\x1b[0m"
        )
        .expect("write to String");
    }
}

pub fn print_strategy_and_providers(
    out: &mut String,
    router_config: &crate::config::router::RouterConfig,
) {
    let Some(balance) =
        router_config.load_balance.as_ref().get(&EndpointType::Chat)
    else {
        return;
    };

    writeln!(out, "  Strategy : \x1b[33m{}\x1b[0m", balance.as_ref())
        .expect("write to String");
    let (label, providers) = provider_order::providers_for_display(balance);
    write!(out, "  {label}: ").expect("write to String");

    for (i, provider) in providers.iter().enumerate() {
        if i > 0 {
            write!(out, ", ").expect("write to String");
        }
        write!(out, "\x1b[32m{provider}\x1b[0m").expect("write to String");
    }
    writeln!(out).expect("write to String");
}
