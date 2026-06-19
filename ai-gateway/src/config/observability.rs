use serde::{Deserialize, Serialize};

use crate::utils::default_true;

/// Provider observability: OTEL attempt metrics, runtime REST snapshot,
/// response JSON header.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(default, rename_all = "kebab-case")]
pub struct ObservabilityConfig {
    #[serde(default = "default_true")]
    pub estimate_tokens: bool,
    pub response_headers: ObservabilityResponseHeadersConfig,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            estimate_tokens: true,
            response_headers: ObservabilityResponseHeadersConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(default, rename_all = "kebab-case")]
pub struct ObservabilityResponseHeadersConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub echo_work_unit_id: bool,
}

impl Default for ObservabilityResponseHeadersConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            echo_work_unit_id: true,
        }
    }
}
