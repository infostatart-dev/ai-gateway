use http::{HeaderValue, Response};

use crate::{
    config::credentials::ProviderCredentialId,
    types::{extensions::RoutedModelAndProvider, model_id::ModelId},
};

pub const REAL_MODE_MODEL_AND_PROVIDER: &str = "X-RealMode-Model-And-Provider";

pub fn format_routed_identity(
    credential_id: &ProviderCredentialId,
    model: &ModelId,
) -> String {
    format!("{credential_id}/{model}")
}

pub fn attach_routed_identity<B>(
    response: &mut Response<B>,
    credential_id: &ProviderCredentialId,
    model: &ModelId,
) {
    let identity = format_routed_identity(credential_id, model);
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
    use crate::types::provider::InferenceProvider;

    #[test]
    fn formats_credential_and_model() {
        let identity = format_routed_identity(
            &ProviderCredentialId::new("openrouter-default"),
            &ModelId::from_str("openai/gpt-oss-120b:free").unwrap(),
        );
        assert_eq!(identity, "openrouter-default/gpt-oss-120b:free");
    }

    #[test]
    fn legacy_provider_format_still_parseable() {
        let _provider = InferenceProvider::OpenRouter;
        let identity = format_routed_identity(
            &ProviderCredentialId::new("gemini-free"),
            &ModelId::from_str("gemini-2.5-flash").unwrap(),
        );
        assert!(identity.starts_with("gemini-free/"));
    }
}
