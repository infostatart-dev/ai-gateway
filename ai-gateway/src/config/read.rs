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

        let mut secrets =
            crate::config::secrets_file::SecretsFile::load_discovered();
        config.credentials =
            crate::config::credentials::CredentialRegistry::build(
                &config.providers,
                &mut secrets,
            );
        apply_integrations(&mut config, &secrets);
        crate::config::secrets_file::SecretsFile::install(secrets);

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

        Ok(config)
    }
}

fn apply_integrations(
    config: &mut Config,
    secrets: &crate::config::secrets_file::SecretsFile,
) {
    if let Some(helicone) = secrets.integrations.helicone.as_ref()
        && !helicone.api_key.is_empty()
    {
        config.helicone.api_key = Secret::from(helicone.api_key.clone());
    }
    if let Some(aws) = secrets.integrations.aws.as_ref()
        && let Some(provider) =
            config.providers.get_mut(&InferenceProvider::Bedrock)
    {
        let url =
            format!("https://bedrock-runtime.{}.amazonaws.com", aws.region);
        if let Ok(parsed) = Url::parse(&url) {
            provider.base_url = parsed;
        }
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
    let mut order = vec![
        InferenceProvider::Named("opencode".into()),
        InferenceProvider::OpenRouter,
        InferenceProvider::Named("github-models".into()),
        InferenceProvider::Named("mistral".into()),
        InferenceProvider::Named("groq".into()),
        InferenceProvider::Named("cerebras".into()),
        InferenceProvider::Named("cloudflare".into()),
        InferenceProvider::GoogleGemini,
    ];
    order.push(InferenceProvider::Named("deepseek-web".into()));
    order.extend([InferenceProvider::Anthropic, InferenceProvider::OpenAI]);
    order.push(InferenceProvider::Named("chatgpt-web".into()));
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
    if crate::config::chatgpt_web::is_chatgpt_web(provider)
        || crate::config::deepseek_web::is_deepseek_web(provider)
    {
        return credentials.has_for(provider);
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

    fn registry_from_secrets(
        yaml: &str,
    ) -> crate::config::credentials::CredentialRegistry {
        let dir = std::env::temp_dir()
            .join(format!("ai-gw-read-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("secrets.yaml");
        std::fs::write(&path, yaml).unwrap();
        let mut secrets =
            crate::config::secrets_file::SecretsFile::load(&path).unwrap();
        let registry = crate::config::credentials::CredentialRegistry::build(
            &ProvidersConfig::default(),
            &mut secrets,
        );
        let _ = std::fs::remove_dir_all(&dir);
        registry
    }

    #[test]
    fn autodefault_includes_gemini_when_any_free_slot_resolves() {
        let credentials = registry_from_secrets(
            "credentials:\n  gemini-free-3:\n    api-key: free-3-key\n",
        );
        let gemini = InferenceProvider::GoogleGemini;
        assert!(is_available_for_autodefault(
            &gemini,
            &ProvidersConfig::default(),
            &credentials,
        ));
    }

    #[test]
    fn autodefault_excludes_opencode_without_secrets_entry() {
        let credentials = registry_from_secrets("credentials: {}\n");
        let opencode = InferenceProvider::Named("opencode".into());
        assert!(ProvidersConfig::default().contains_key(&opencode));
        assert!(!is_available_for_autodefault(
            &opencode,
            &ProvidersConfig::default(),
            &credentials,
        ));
    }

    #[test]
    fn autodefault_includes_opencode_with_secrets_entry() {
        let credentials = registry_from_secrets(
            "credentials:\n  opencode-default:\n    api-key: test-key\n",
        );
        let opencode = InferenceProvider::Named("opencode".into());
        assert!(is_available_for_autodefault(
            &opencode,
            &ProvidersConfig::default(),
            &credentials,
        ));
    }

    #[test]
    fn autodefault_includes_github_models_when_credential_set() {
        let providers = ProvidersConfig::default();
        let empty = registry_from_secrets("credentials: {}\n");
        let github = InferenceProvider::Named("github-models".into());
        assert!(providers.contains_key(&github));
        assert!(!is_available_for_autodefault(&github, &providers, &empty));

        let credentials = registry_from_secrets(
            r#"
credentials:
  openrouter-default:
    api-key: sk-or-test
  github-models-default:
    api-key: ghp_test
  mistral-default:
    api-key: mistral-test
"#,
        );
        assert!(is_available_for_autodefault(
            &github,
            &providers,
            &credentials
        ));

        let config = Config {
            providers: providers.clone(),
            credentials: credentials.clone(),
            ..Config::default()
        };
        let router =
            build_autodefault_router(&config).expect("autodefault router");
        let strategy = router.load_balance.0.get(&EndpointType::Chat).unwrap();
        let BalanceConfigInner::BudgetAwareCapabilityAfter {
            provider_priorities,
            ..
        } = strategy
        else {
            panic!("expected BudgetAwareCapabilityAfter");
        };
        let openrouter_rank = provider_priorities
            .get(&InferenceProvider::OpenRouter)
            .copied()
            .expect("openrouter in autodefault");
        let github_rank = provider_priorities
            .get(&github)
            .copied()
            .expect("github-models in autodefault");
        let mistral_rank = provider_priorities
            .get(&InferenceProvider::Named("mistral".into()))
            .copied()
            .expect("mistral in autodefault");
        assert!(openrouter_rank < github_rank);
        assert!(github_rank < mistral_rank);
    }
}
