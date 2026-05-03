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
        router::RouterConfig,
    },
    endpoints::EndpointType,
    types::{
        provider::{InferenceProvider, ProviderKey},
        secret::Secret,
    },
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
    let providers: HashSet<_> = config
        .providers
        .keys()
        .filter(|provider| is_available_for_autodefault(provider))
        .cloned()
        .collect();
    let providers = nonempty_collections::NESet::try_from_set(providers)?;
    Some(build_autodefault_router_config(
        providers,
        config.decision.enabled,
    ))
}

fn build_autodefault_router_config(
    providers: nonempty_collections::NESet<InferenceProvider>,
    decision_enabled: bool,
) -> RouterConfig {
    let strategy = if decision_enabled {
        BalanceConfigInner::BudgetAware {
            providers,
            provider_priorities: IndexMap::new(),
            max_cooldown_wait: default_budget_max_cooldown_wait(),
        }
    } else {
        BalanceConfigInner::CapabilityAware { providers }
    };

    RouterConfig {
        load_balance: BalanceConfig(HashMap::from([(
            EndpointType::Chat,
            strategy,
        )])),
        ..Default::default()
    }
}

fn is_available_for_autodefault(provider: &InferenceProvider) -> bool {
    ProviderKey::from_env(provider).is_some()
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

        assert!(config.decision.enabled);
        assert!(matches!(strategy, BalanceConfigInner::BudgetAware { .. }));
    }

    #[test]
    fn autodefault_uses_budget_aware_when_decision_is_enabled() {
        let router = build_autodefault_router_config(
            nonempty_collections::nes![InferenceProvider::Named("groq".into())],
            true,
        );
        let strategy = router.load_balance.0.get(&EndpointType::Chat).unwrap();

        assert!(matches!(strategy, BalanceConfigInner::BudgetAware { .. }));
    }
}
