use crate::config::credentials::ProviderCredentialId;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RouteBinding {
    pub credential_id: ProviderCredentialId,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteBindingPreference {
    pub binding: RouteBinding,
    pub score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteStreamMode {
    NonStreaming,
    Streaming,
}

impl RouteStreamMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NonStreaming => "non-stream",
            Self::Streaming => "stream",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RouteMemoryKey {
    value: String,
}

impl RouteMemoryKey {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    #[must_use]
    pub fn for_route(
        requirements: &crate::router::capability::RequestRequirements,
        intent: Option<crate::router::intent::RoutingIntent>,
    ) -> Self {
        let intent_tier =
            intent.map_or("none", |intent| intent.preferred_tier.as_str());
        let context_bucket = requirements
            .min_context_tokens
            .map_or("none", context_bucket);
        Self::new(format!(
            "intent={intent_tier}|json_schema={}|context={context_bucket}",
            requirements.json_schema_required
        ))
    }

    #[must_use]
    pub fn for_route_class(
        router_id: &crate::types::router::RouterId,
        endpoint_type: crate::endpoints::EndpointType,
        requirements: &crate::router::capability::RequestRequirements,
        intent: Option<crate::router::intent::RoutingIntent>,
        source_model: Option<&str>,
        stream: RouteStreamMode,
    ) -> Self {
        let intent_tier =
            intent.map_or("none", |intent| intent.preferred_tier.as_str());
        let context_bucket = requirements
            .min_context_tokens
            .map_or("none", context_bucket);
        let source_model = source_model.unwrap_or("none");
        Self::new(format!(
            "router={}|endpoint={}|source_model={source_model}|strict_json={}|stream={}|context={context_bucket}|stability={intent_tier}",
            router_id.as_ref(),
            endpoint_type.as_ref(),
            requirements.json_schema_required,
            stream.as_str(),
        ))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

fn context_bucket(tokens: u32) -> &'static str {
    match tokens {
        0..=8_191 => "small",
        8_192..=32_767 => "medium",
        32_768..=131_071 => "large",
        _ => "huge",
    }
}

#[cfg(test)]
mod tests {
    use compact_str::CompactString;

    use super::*;
    use crate::{
        endpoints::EndpointType,
        router::{
            capability::RequestRequirements,
            intent::{IntentTier, RoutingIntent},
        },
        types::router::RouterId,
    };

    #[test]
    fn route_class_key_separates_router_source_stream_and_json() {
        let router_a = RouterId::Named(CompactString::new("autodefault"));
        let router_b = RouterId::Named(CompactString::new("managed-openai"));
        let source = "openai/gpt-5-mini";
        let intent = Some(RoutingIntent {
            preferred_tier: IntentTier::Deep,
            ..RoutingIntent::default()
        });
        let strict = RequestRequirements {
            json_schema_required: true,
            min_context_tokens: Some(32_000),
            ..RequestRequirements::default()
        };
        let loose = RequestRequirements {
            json_schema_required: false,
            min_context_tokens: Some(32_000),
            ..RequestRequirements::default()
        };

        let baseline = RouteMemoryKey::for_route_class(
            &router_a,
            EndpointType::Chat,
            &strict,
            intent,
            Some(source),
            RouteStreamMode::NonStreaming,
        );
        let other_router = RouteMemoryKey::for_route_class(
            &router_b,
            EndpointType::Chat,
            &strict,
            intent,
            Some(source),
            RouteStreamMode::NonStreaming,
        );
        let other_json = RouteMemoryKey::for_route_class(
            &router_a,
            EndpointType::Chat,
            &loose,
            intent,
            Some(source),
            RouteStreamMode::NonStreaming,
        );
        let other_stream = RouteMemoryKey::for_route_class(
            &router_a,
            EndpointType::Chat,
            &strict,
            intent,
            Some(source),
            RouteStreamMode::Streaming,
        );

        assert_ne!(baseline, other_router);
        assert_ne!(baseline, other_json);
        assert_ne!(baseline, other_stream);
    }
}
