use std::sync::Arc;

use tokio::sync::mpsc::Sender;
use tower::ServiceBuilder;

use super::{Dispatcher, DispatcherService, DispatcherServiceWithoutMapper};
use crate::{
    app_state::AppState,
    config::router::RouterConfig,
    dispatcher::client::Client,
    error::init::InitError,
    middleware::{
        add_extension::AddExtensionsLayer,
        mapper::{model::ModelMapper, registry::EndpointConverterRegistry},
    },
    types::{
        model_id::ModelId, provider::InferenceProvider,
        rate_limit::RateLimitEvent, router::RouterId,
    },
    utils::handle_error::ErrorHandlerLayer,
};

impl Dispatcher {
    async fn new_inner(
        app_state: AppState,
        router_id: &RouterId,
        provider: InferenceProvider,
        model_mapper: ModelMapper,
        rate_limit_tx: Option<Sender<RateLimitEvent>>,
    ) -> Result<DispatcherService, InitError> {
        let client = Client::new(&app_state, provider.clone()).await?;

        let dispatcher = Self {
            client,
            app_state: app_state.clone(),
            provider: provider.clone(),
            rate_limit_tx,
        };
        let converter_registry = EndpointConverterRegistry::new(&model_mapper);
        let extensions_layer = AddExtensionsLayer::builder()
            .inference_provider(provider.clone())
            .router_id(Some(router_id.clone()))
            .build();

        Ok(ServiceBuilder::new()
            .layer(extensions_layer)
            .layer(ErrorHandlerLayer::new(app_state))
            .layer(crate::middleware::mapper::Layer::new(converter_registry))
            .service(dispatcher))
    }

    pub async fn new(
        app_state: AppState,
        router_id: &RouterId,
        router_config: &Arc<RouterConfig>,
        provider: InferenceProvider,
    ) -> Result<DispatcherService, InitError> {
        let model_mapper = ModelMapper::new_for_router(
            app_state.clone(),
            router_config.clone(),
        );
        let rate_limit_tx = app_state.get_rate_limit_tx(router_id).await?;
        Self::new_inner(
            app_state,
            router_id,
            provider,
            model_mapper,
            Some(rate_limit_tx),
        )
        .await
    }

    pub async fn new_without_rate_limit_events(
        app_state: AppState,
        router_id: &RouterId,
        router_config: &Arc<RouterConfig>,
        provider: InferenceProvider,
    ) -> Result<DispatcherService, InitError> {
        let model_mapper = ModelMapper::new_for_router(
            app_state.clone(),
            router_config.clone(),
        );
        Self::new_inner(app_state, router_id, provider, model_mapper, None)
            .await
    }

    pub async fn new_with_model_id_without_rate_limit_events(
        app_state: AppState,
        router_id: &RouterId,
        router_config: &Arc<RouterConfig>,
        provider: InferenceProvider,
        model_id: ModelId,
    ) -> Result<DispatcherService, InitError> {
        let model_mapper = ModelMapper::new_with_model_id(
            app_state.clone(),
            router_config.clone(),
            model_id,
        );
        Self::new_inner(app_state, router_id, provider, model_mapper, None)
            .await
    }

    pub async fn new_with_model_id(
        app_state: AppState,
        router_id: &RouterId,
        router_config: &Arc<RouterConfig>,
        provider: InferenceProvider,
        model_id: ModelId,
    ) -> Result<DispatcherService, InitError> {
        let model_mapper = ModelMapper::new_with_model_id(
            app_state.clone(),
            router_config.clone(),
            model_id,
        );
        let rate_limit_tx = app_state.get_rate_limit_tx(router_id).await?;
        Self::new_inner(
            app_state,
            router_id,
            provider,
            model_mapper,
            Some(rate_limit_tx),
        )
        .await
    }

    pub async fn new_direct_proxy(
        app_state: AppState,
        provider: &InferenceProvider,
    ) -> Result<DispatcherService, InitError> {
        let client = Client::new(&app_state, provider.clone()).await?;
        let dispatcher = Self {
            client,
            app_state: app_state.clone(),
            provider: provider.clone(),
            rate_limit_tx: None,
        };
        let model_mapper = ModelMapper::new(app_state.clone());
        let converter_registry = EndpointConverterRegistry::new(&model_mapper);
        let extensions_layer = AddExtensionsLayer::builder()
            .inference_provider(provider.clone())
            .router_id(None)
            .build();

        Ok(ServiceBuilder::new()
            .layer(extensions_layer)
            .layer(ErrorHandlerLayer::new(app_state))
            .layer(crate::middleware::mapper::Layer::new(converter_registry))
            .service(dispatcher))
    }

    pub async fn new_without_mapper(
        app_state: AppState,
        provider: &InferenceProvider,
    ) -> Result<DispatcherServiceWithoutMapper, InitError> {
        let client = Client::new(&app_state, provider.clone()).await?;
        let dispatcher = Self {
            client,
            app_state: app_state.clone(),
            provider: provider.clone(),
            rate_limit_tx: None,
        };
        let extensions_layer = AddExtensionsLayer::builder()
            .inference_provider(provider.clone())
            .router_id(None)
            .build();

        Ok(ServiceBuilder::new()
            .layer(extensions_layer)
            .layer(ErrorHandlerLayer::new(app_state))
            .service(dispatcher))
    }
}
