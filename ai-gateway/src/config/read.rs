use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use indexmap::IndexMap;
use json_patch::merge;
use url::Url;

use crate::{
    config::{
        Config, DEFAULT_CONFIG_PATH, Error,
        balance::{
            BalanceConfig, BalanceConfigInner, default_budget_max_cooldown_wait,
        },
        decision::{RouterDecisionConfig, TierCascade},
        providers::ProvidersConfig,
        router::RouterConfig,
    },
    endpoints::EndpointType,
    types::{provider::InferenceProvider, secret::Secret},
};

impl Config {
    pub fn try_read(
        config_file_path: Option<PathBuf>,
    ) -> Result<Self, Box<Error>> {
        let mut default_config = serde_json::to_value(Self::default())
            .expect("default config is serializable");
        let mut builder = config::Config::builder();
        if let Some(path) = config_file_path {
            builder = builder.add_source(config::File::from(path));
        } else if std::fs::exists(DEFAULT_CONFIG_PATH).unwrap_or_default() {
            builder = builder.add_source(config::File::from(PathBuf::from(
                DEFAULT_CONFIG_PATH,
            )));
        }
        builder = builder.add_source(
            config::Environment::with_prefix("AI_GATEWAY")
                .try_parsing(true)
                .separator("__")
                .convert_case(config::Case::Kebab),
        );
        let input_config: serde_json::Value = builder
            .build()
            .map_err(Error::from)
            .map_err(Box::new)?
            .try_deserialize()
            .map_err(Error::from)
            .map_err(Box::new)?;
        merge(&mut default_config, &input_config);
        let mut config: Config =
            serde_path_to_error::deserialize(default_config)
                .map_err(Error::from)
                .map_err(Box::new)?;
        config.credentials =
            crate::config::credentials::CredentialRegistry::build(
                &config.providers,
            );

        let autodefault_id = Self::autodefault_router_id();
        if config.deployment_target.is_sidecar()
            && config.routers.contains_key(&autodefault_id)
        {
            return Err(Box::new(Error::ReservedRouterId(
                "autodefault".into(),
            )));
        }

        if config.deployment_target.is_sidecar()
            && let Some(autodefault_router) = build_autodefault_router(&config)
        {
            config
                .routers
                .as_mut()
                .insert(autodefault_id, autodefault_router);
        }

        if let Ok(k) = std::env::var("HELICONE_CONTROL_PLANE_API_KEY") {
            config.helicone.api_key = Secret::from(k);
        }
        if let Ok(reg) = std::env::var("AWS_REGION")
            && let Some(p) =
                config.providers.get_mut(&InferenceProvider::Bedrock)
        {
            let url = format!("https://bedrock-runtime.{reg}.amazonaws.com");
            p.base_url = Url::parse(&url).map_err(Error::UrlParse)?;
        }
        Ok(config)
    }
}

fn build_autodefault_router(config: &Config) -> Option<RouterConfig> {
    let mut providers = Vec::new();
    let mut provider_priorities = IndexMap::new();

    for (rank, provider) in autodefault_provider_order().into_iter().enumerate()
    {
        if is_available_for_autodefault(
            &provider,
            &config.providers,
            &config.credentials,
        ) {
            provider_priorities.insert(
                provider.clone(),
                u16::try_from(rank).unwrap_or(u16::MAX),
            );
            providers.push(provider);
        }
    }

    let providers = nonempty_collections::NESet::try_from_set(
        providers.into_iter().collect::<HashSet<_>>(),
    )?;
    Some(build_autodefault_router_config(
        providers,
        provider_priorities,
    ))
}

fn autodefault_provider_order() -> Vec<InferenceProvider> {
    let mut order = Vec::new();
    if crate::config::chatgpt_web::session_file_available() {
        order.push(InferenceProvider::Named("chatgpt-web".into()));
    }
    order.extend([
        InferenceProvider::Named("opencode".into()),
        InferenceProvider::OpenRouter,
        InferenceProvider::Named("mistral".into()),
        InferenceProvider::Named("groq".into()),
        InferenceProvider::Named("cerebras".into()),
        InferenceProvider::Named("cloudflare".into()),
        InferenceProvider::GoogleGemini,
        InferenceProvider::Anthropic,
    ]);
    order
}

fn build_autodefault_router_config(
    providers: nonempty_collections::NESet<InferenceProvider>,
    provider_priorities: IndexMap<InferenceProvider, u16>,
) -> RouterConfig {
    let strategy = BalanceConfigInner::BudgetAwareCapabilityAfter {
        providers,
        provider_priorities,
        max_cooldown_wait: default_budget_max_cooldown_wait(),
    };

    RouterConfig {
        load_balance: BalanceConfig(HashMap::from([(
            EndpointType::Chat,
            strategy,
        )])),
        decision: RouterDecisionConfig {
            enabled: true,
            tier_cascade: Some(TierCascade::FreeUp),
        },
        ..Default::default()
    }
}

fn is_available_for_autodefault(
    provider: &InferenceProvider,
    providers_config: &ProvidersConfig,
    credentials: &crate::config::credentials::CredentialRegistry,
) -> bool {
    if !providers_config.contains_key(provider) {
        return false;
    }
    if crate::config::chatgpt_web::is_chatgpt_web(provider) {
        return crate::config::chatgpt_web::session_file_available();
    }
    provider.is_keyless() || credentials.has_for(provider)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use compact_str::CompactString;

    use super::*;
    use crate::types::router::RouterId;

    #[test]
    fn decision_example_config_loads_budget_aware_router() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("config/decision-example.yaml");
        let config = Config::try_read(Some(path)).unwrap();
        let router_id = RouterId::Named(CompactString::new("decision"));
        let router = config.routers.get(&router_id).unwrap();
        let strategy = router.load_balance.0.get(&EndpointType::Chat).unwrap();

        assert!(router.decision.enabled);
        assert!(matches!(strategy, BalanceConfigInner::BudgetAware { .. }));
    }

    #[test]
    fn autodefault_uses_budget_then_capability() {
        let router = build_autodefault_router_config(
            nonempty_collections::nes![InferenceProvider::Named("groq".into())],
            IndexMap::new(),
        );
        let strategy = router.load_balance.0.get(&EndpointType::Chat).unwrap();

        assert!(matches!(
            strategy,
            BalanceConfigInner::BudgetAwareCapabilityAfter { .. }
        ));
        assert!(router.decision.enabled);
        assert_eq!(router.decision.tier_cascade, Some(TierCascade::FreeUp));
    }

    #[serial_test::serial(env)]
    #[test]
    fn autodefault_includes_gemini_when_any_free_slot_resolves() {
        unsafe {
            std::env::remove_var("AI_GATEWAY_CREDENTIAL_GEMINI_FREE");
            std::env::remove_var("GEMINI_FREE_TIER_API_KEY");
            std::env::remove_var("GEMINI_FREE_TIER_APIKEY");
            std::env::remove_var("GEMINI_API_KEY");
            std::env::set_var(
                "AI_GATEWAY_CREDENTIAL_GEMINI_FREE_3",
                "free-3-key",
            );
        }
        let config = Config::default();
        let gemini = InferenceProvider::GoogleGemini;

        assert!(
            is_available_for_autodefault(
                &gemini,
                &config.providers,
                &config.credentials,
            ),
            "gemini must join autodefault when any free sibling slot resolves"
        );

        unsafe {
            std::env::remove_var("AI_GATEWAY_CREDENTIAL_GEMINI_FREE_3");
        }
    }

    #[serial_test::serial(env)]
    #[test]
    fn autodefault_excludes_opencode_without_api_key() {
        // SAFETY: serialized test; restores env before exit.
        unsafe {
            std::env::remove_var("OPENCODE_API_KEY");
        }
        let config = Config::default();
        let opencode = InferenceProvider::Named("opencode".into());

        assert!(
            config.providers.contains_key(&opencode),
            "embedded providers must include opencode"
        );
        assert!(
            !is_available_for_autodefault(
                &opencode,
                &config.providers,
                &config.credentials,
            ),
            "opencode must be omitted from autodefault without \
             OPENCODE_API_KEY"
        );

        unsafe {
            std::env::set_var("OPENCODE_API_KEY", "test-key");
        }
        let config = Config::default();
        assert!(
            is_available_for_autodefault(
                &opencode,
                &config.providers,
                &config.credentials,
            ),
            "opencode must join autodefault when OPENCODE_API_KEY is set"
        );
        unsafe {
            std::env::remove_var("OPENCODE_API_KEY");
        }
    }
}
