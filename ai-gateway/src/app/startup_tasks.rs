//! Meltdown service names registered at HTTP startup.
//!
//! Helicone Cloud control-plane websocket is **not** on this list until the
//! Infostart-owned control plane ships — see `docs/control-plane.md`.

use crate::config::Config;

/// Returns meltdown service names for `run_app` (excludes
/// `control-plane-client`).
#[must_use]
pub fn meltdown_service_names(config: &Config) -> Vec<&'static str> {
    let mut tasks = vec![
        "shutdown-signals",
        "gateway",
        "provider-health-monitor",
        "provider-rate-limit-monitor",
        "system-metrics",
    ];

    if config.deployment_target.is_cloud() {
        tasks.push("database-listener");
    }

    if config.global.rate_limit.is_some() {
        tasks.push("rate-limiting-cleanup");
    }

    if config.client_access.enabled {
        tasks.push("client-access-reloader");
    }

    tasks
}

#[cfg(test)]
mod tests {
    use super::meltdown_service_names;
    use crate::config::{
        Config,
        helicone::{HeliconeConfig, HeliconeFeatures},
    };

    #[test]
    fn sidecar_helicone_features_all_omits_control_plane_client() {
        let mut config = Config::default();
        config.helicone = HeliconeConfig {
            features: HeliconeFeatures::All,
            ..HeliconeConfig::default()
        };

        let names = meltdown_service_names(&config);

        assert!(names.contains(&"gateway"));
        assert!(!names.contains(&"control-plane-client"));
    }
}
