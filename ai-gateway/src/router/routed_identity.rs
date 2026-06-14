use http::{HeaderValue, Response};

use crate::types::{
    extensions::RoutedModelAndProvider,
    model_id::ModelId,
    provider::InferenceProvider,
};

pub const REAL_MODE_MODEL_AND_PROVIDER: &str = "X-RealMode-Model-And-Provider";

pub fn format_routed_identity(
    provider: &InferenceProvider,
    model: &ModelId,
) -> String {
    format!("{provider}/{model}")
}

pub fn attach_routed_identity<B>(
    response: &mut Response<B>,
    provider: &InferenceProvider,
    model: &ModelId,
) {
    let identity = format_routed_identity(provider, model);
    response
        .extensions_mut()
        .insert(RoutedModelAndProvider(identity.clone()));
    if let Ok(header_value) = HeaderValue::from_str(&identity) {
        response.headers_mut().insert(
            http::HeaderName::from_static("x-realmode-model-and-provider"),
            header_value,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn formats_provider_and_model() {
        let identity = format_routed_identity(
            &InferenceProvider::OpenRouter,
            &ModelId::from_str("openai/gpt-oss-120b:free").unwrap(),
        );
        assert_eq!(identity, "openrouter/gpt-oss-120b:free");
    }
}
