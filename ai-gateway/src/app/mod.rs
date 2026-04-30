use crate::{app_state::AppState, config::Config, error::init::InitError};
use std::convert::Infallible;
use tower::util::BoxCloneService;

pub mod cache;
pub mod factory;
pub mod run;
pub mod service;
pub mod stack;
pub mod state;

pub type AppResponseBody = tower_http::body::UnsyncBoxBody<
    bytes::Bytes,
    Box<dyn std::error::Error + Send + Sync + 'static>,
>;
pub type AppResponse = http::Response<AppResponseBody>;

pub type BoxedServiceStack =
    BoxCloneService<crate::types::request::Request, AppResponse, Infallible>;
pub type BoxedHyperServiceStack = BoxCloneService<
    http::Request<hyper::body::Incoming>,
    AppResponse,
    Infallible,
>;

#[derive(Clone)]
pub struct App {
    pub state: AppState,
    pub service_stack: BoxedServiceStack,
}

impl App {
    pub async fn new(config: Config) -> Result<Self, InitError> {
        let app_state = state::build_app_state(config).await?;
        let service_stack =
            stack::build_service_stack(app_state.clone()).await?;
        Ok(Self {
            state: app_state,
            service_stack,
        })
    }
}
