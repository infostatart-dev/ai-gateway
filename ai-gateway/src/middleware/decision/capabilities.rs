use crate::types::provider::InferenceProvider;

/// Defines the minimum capabilities required by a request to be serviced by a model.
#[derive(Debug, Clone, Default)]
pub struct ModelCapabilities {
    pub max_context: usize,
    pub supports_tools: bool,
    pub supports_json: bool,
}

impl ModelCapabilities {
    /// Evaluates if the current capabilities meet the requirements of a specific request.
    /// A robust implementation would compare requested features (tools used, JSON format requested,
    /// context length estimated) against these capabilities.
    pub fn can_handle(&self, request_context_length: usize, requires_tools: bool, requires_json: bool) -> bool {
        if request_context_length > self.max_context {
            return false;
        }
        if requires_tools && !self.supports_tools {
            return false;
        }
        if requires_json && !self.supports_json {
            return false;
        }
        true
    }
}

/// Capability filter that ensures we do not route a request to a provider that lacks necessary capabilities.
pub struct CapabilityFilter;

impl CapabilityFilter {
    /// Filter out models that cannot handle the request.
    pub fn filter_candidates<'a>(
        request_context_length: usize,
        requires_tools: bool,
        requires_json: bool,
        candidates: impl Iterator<Item = (&'a InferenceProvider, &'a ModelCapabilities)>,
    ) -> Vec<&'a InferenceProvider> {
        candidates
            .filter_map(|(provider, caps)| {
                if caps.can_handle(request_context_length, requires_tools, requires_json) {
                    Some(provider)
                } else {
                    None
                }
            })
            .collect()
    }
}
