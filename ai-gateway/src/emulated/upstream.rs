use url::Url;

use crate::config::Config;

#[must_use]
pub fn emulated_enabled() -> bool {
    std::env::var("AI_GATEWAY_EMULATED")
        .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

#[must_use]
pub fn emulator_base_url() -> String {
    std::env::var("AI_GATEWAY_EMULATOR_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:5151".into())
}

pub fn apply_upstream_binding(config: &mut Config) {
    let base = emulator_base_url();
    for (provider, provider_cfg) in config.providers.iter_mut() {
        if is_browser_session(provider, &config.provider_limits) {
            continue;
        }
        let rewritten =
            rewrite_base_url(&base, provider.as_ref(), &provider_cfg.base_url);
        provider_cfg.base_url = rewritten;
    }
}

fn is_browser_session(
    provider: &crate::types::provider::InferenceProvider,
    limits: &crate::config::provider_limits::ProviderLimitCatalog,
) -> bool {
    limits.provider(provider).and_then(|c| c.scope.as_deref())
        == Some("browser-session")
}

fn rewrite_base_url(
    emulator_base: &str,
    provider_id: &str,
    original: &Url,
) -> Url {
    let base = emulator_base.trim_end_matches('/');
    let path = original.path();
    let joined = if path == "/" {
        format!("{base}/{provider_id}/")
    } else {
        format!("{base}/{provider_id}{path}")
    };
    Url::parse(&joined).unwrap_or_else(|_| original.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_preserving_path_suffix() {
        let emulator = "http://127.0.0.1:5151";
        let cases = [
            (
                "provider-a",
                "https://upstream.example/openai/",
                "http://127.0.0.1:5151/provider-a/openai/",
            ),
            (
                "provider-b",
                "https://upstream.example/api/v1/",
                "http://127.0.0.1:5151/provider-b/api/v1/",
            ),
            (
                "provider-c",
                "https://upstream.example/",
                "http://127.0.0.1:5151/provider-c/",
            ),
        ];
        for (id, original, want) in cases {
            let url = rewrite_base_url(
                emulator,
                id,
                &Url::parse(original).expect("original url"),
            );
            assert_eq!(url.as_str(), want, "provider_id={id}");
        }
    }
}
