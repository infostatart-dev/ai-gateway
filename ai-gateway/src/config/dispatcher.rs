use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct DispatcherConfig {
    #[serde(default = "default_timeout", with = "humantime_serde")]
    pub timeout: Duration,
    #[serde(default = "default_connection_timeout", with = "humantime_serde")]
    pub connection_timeout: Duration,
    /// Pass-through to `reqwest::ClientBuilder::gzip`; default true.
    #[serde(
        default = "default_gzip_decompress_responses",
        rename = "gzip-decompress-responses"
    )]
    pub gzip_decompress_responses: bool,
}

impl Default for DispatcherConfig {
    fn default() -> Self {
        Self {
            timeout: default_timeout(),
            connection_timeout: default_connection_timeout(),
            gzip_decompress_responses: default_gzip_decompress_responses(),
        }
    }
}

#[cfg(feature = "testing")]
impl crate::tests::TestDefault for DispatcherConfig {
    fn test_default() -> Self {
        Self::default()
    }
}

fn default_timeout() -> Duration {
    Duration::from_mins(15)
}

fn default_connection_timeout() -> Duration {
    Duration::from_secs(10)
}

fn default_gzip_decompress_responses() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gzip_decompress_defaults_true() {
        assert!(DispatcherConfig::default().gzip_decompress_responses);
    }

    #[test]
    fn gzip_decompress_yaml_alias() {
        let yaml = r"
timeout: 1m
connection-timeout: 10s
gzip-decompress-responses: false
";
        let cfg: DispatcherConfig = serde_yml::from_str(yaml).unwrap();
        assert!(!cfg.gzip_decompress_responses);
    }
}
