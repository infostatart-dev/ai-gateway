use http::{HeaderMap, HeaderName, HeaderValue};
use reqwest::ClientBuilder;

use crate::{
    app_state::AppState,
    error::{init::InitError, provider::ProviderError},
    types::{
        provider::{InferenceProvider, ProviderKey},
        secret::Secret,
    },
    utils::host_header,
};

#[derive(Debug, Clone, Default)]
pub struct Client(pub(super) reqwest::Client);

impl Client {
    pub fn new(
        app_state: &AppState,
        client_builder: ClientBuilder,
        provider: &InferenceProvider,
        provider_key: Option<&ProviderKey>,
    ) -> Result<Self, InitError> {
        let provider_config =
            app_state.0.config.providers.get(provider).ok_or_else(|| {
                ProviderError::ProviderNotConfigured(provider.clone())
            })?;

        let base_url = provider_config.base_url.clone();

        let mut default_headers = HeaderMap::new();
        if let Some(ProviderKey::Secret(key)) = provider_key {
            default_headers.insert(
                http::header::AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", key.expose()))
                    .unwrap(),
            );
        }
        for (name, value) in &provider_config.request_headers {
            default_headers.insert(
                HeaderName::from_bytes(name.as_bytes()).expect(
                    "embedded provider request-headers use valid header names",
                ),
                HeaderValue::from_str(value).expect(
                    "embedded provider request-headers use valid header values",
                ),
            );
        }
        default_headers.insert(http::header::HOST, host_header(&base_url));
        default_headers.insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_str(mime::APPLICATION_JSON.essence_str())
                .unwrap(),
        );
        let inner = client_builder
            .default_headers(default_headers)
            .build()
            .map_err(InitError::CreateReqwestClient)?;
        Ok(Self(inner))
    }

    pub fn set_auth_header(
        request_builder: reqwest::RequestBuilder,
        key: &Secret<String>,
    ) -> reqwest::RequestBuilder {
        request_builder.header(
            http::header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", key.expose())).unwrap(),
        )
    }
}
