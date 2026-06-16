use std::{fmt::Write as _, net::SocketAddr};

use crate::{
    cli::{banner, router_summary},
    config::Config,
};

const DEFAULT_AUTODEFAULT_MODEL: &str = "openai/gpt-5.4-nano";

fn autodefault_example_model() -> String {
    std::env::var("AI_GATEWAY_AUTODEFAULT_DEFAULT_MODEL")
        .unwrap_or_else(|_| DEFAULT_AUTODEFAULT_MODEL.to_string())
}

pub fn show_welcome_banner(addr: &SocketAddr, config: &Config) {
    banner::print_ascii_banner();
    print_example_curl(addr);

    if let Some(router_config) = autodefault_router(config) {
        print_autodefault_section(addr, config, router_config);
    }
    router_summary::print_configured_router_sections(config);
}

fn print_example_curl(addr: &SocketAddr) {
    let model = autodefault_example_model();
    println!(
        "Try it out with this example request:\n\n\
         \x1b[0mcurl --request POST \\\n\
         \x20 --url http://{addr:?}/ai/chat/completions \\\n\
         \x20 --header 'Content-Type: application/json' \\\n\
         \x20 --data '{{\n\
         \x20   \"model\": \"{model}\",\n\
         \x20   \"messages\": [\n\
         \x20     {{\n\
         \x20       \"role\": \"user\",\n\
         \x20       \"content\": \"hello world\"\n\
         \x20     }}\n\
         \x20   ]\n\
         \x20 }}'\x1b[0m"
    );
}

fn autodefault_router(
    config: &Config,
) -> Option<&crate::config::router::RouterConfig> {
    if !config.has_autodefault_router() {
        return None;
    }
    config.routers.get(&Config::autodefault_router_id())
}

fn print_autodefault_section(
    addr: &SocketAddr,
    config: &Config,
    router_config: &crate::config::router::RouterConfig,
) {
    let mut section = String::from(
        "\n\n\x1b[1mAutodefault router\x1b[0m  \
         /router/autodefault/chat/completions\n",
    );

    router_summary::print_decision_status(&mut section, config, router_config);
    router_summary::print_strategy_and_providers(&mut section, router_config);
    print_autodefault_curl(&mut section, addr);

    print!("{section}");
}

fn print_autodefault_curl(out: &mut String, addr: &SocketAddr) {
    let model = autodefault_example_model();
    write!(
        out,
        "\n\x1b[0mcurl --request POST \\\n\
         \x20 --url http://{addr:?}/router/autodefault/chat/completions \\\n\
         \x20 --header 'Content-Type: application/json' \\\n\
         \x20 --data '{{\n\
         \x20   \"model\": \"{model}\",\n\
         \x20   \"messages\": [\n\
         \x20     {{\n\
         \x20       \"role\": \"user\",\n\
         \x20       \"content\": \"hello world\"\n\
         \x20     }}\n\
         \x20   ]\n\
         \x20 }}'\x1b[0m\n"
    )
    .expect("write to String");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial_test::serial(env)]
    fn autodefault_example_model_defaults_to_nano() {
        unsafe {
            std::env::remove_var("AI_GATEWAY_AUTODEFAULT_DEFAULT_MODEL");
        }
        assert_eq!(autodefault_example_model(), "openai/gpt-5.4-nano");
    }

    #[test]
    #[serial_test::serial(env)]
    fn autodefault_example_model_respects_env_override() {
        unsafe {
            std::env::set_var(
                "AI_GATEWAY_AUTODEFAULT_DEFAULT_MODEL",
                "openai/gpt-5-mini",
            );
        }
        assert_eq!(autodefault_example_model(), "openai/gpt-5-mini");
        unsafe {
            std::env::remove_var("AI_GATEWAY_AUTODEFAULT_DEFAULT_MODEL");
        }
    }
}
