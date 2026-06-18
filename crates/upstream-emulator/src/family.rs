use ai_gateway::config::providers::GlobalProviderConfig;
use url::Url;

/// Wire/429 response shape — derived from embedded `providers.yaml`, not
/// provider id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolFamily {
    OpenAiCompat,
    GeminiOpenAiCompat,
    AnthropicMessages,
}

pub fn protocol_family(cfg: &GlobalProviderConfig) -> ProtocolFamily {
    if cfg.version.is_some() {
        return ProtocolFamily::AnthropicMessages;
    }
    if is_gemini_upstream(&cfg.base_url) {
        return ProtocolFamily::GeminiOpenAiCompat;
    }
    ProtocolFamily::OpenAiCompat
}

fn is_gemini_upstream(base_url: &Url) -> bool {
    base_url
        .host_str()
        .is_some_and(|host| host.contains("generativelanguage.googleapis.com"))
}

#[cfg(test)]
mod tests {
    use indexmap::{IndexMap, IndexSet};

    use super::*;

    fn cfg(base_url: &str, version: Option<&str>) -> GlobalProviderConfig {
        GlobalProviderConfig {
            models: IndexSet::new(),
            base_url: Url::parse(base_url).expect("url"),
            version: version.map(str::to_string),
            gzip_decompress_responses: None,
            model_capabilities: IndexMap::default(),
            request_headers: IndexMap::default(),
            model_catalog_keys: IndexMap::default(),
            last_verified_at: None,
            verify_source: None,
        }
    }

    #[test]
    fn anthropic_from_version_field() {
        assert_eq!(
            protocol_family(&cfg(
                "https://api.anthropic.com/",
                Some("2023-06-01")
            )),
            ProtocolFamily::AnthropicMessages
        );
    }

    #[test]
    fn gemini_from_base_url_host() {
        assert_eq!(
            protocol_family(&cfg(
                "https://generativelanguage.googleapis.com/",
                None
            )),
            ProtocolFamily::GeminiOpenAiCompat
        );
    }

    #[test]
    fn openai_compat_is_default() {
        assert_eq!(
            protocol_family(&cfg("https://api.groq.com/openai/", None)),
            ProtocolFamily::OpenAiCompat
        );
        assert_eq!(
            protocol_family(&cfg("https://openrouter.ai/api/v1/", None)),
            ProtocolFamily::OpenAiCompat
        );
    }

    #[test]
    fn embedded_catalog_assigns_family_without_provider_match() {
        let providers =
            ai_gateway::config::providers::ProvidersConfig::default();
        for (_id, entry) in providers.iter() {
            let _ = protocol_family(entry);
        }
    }
}
