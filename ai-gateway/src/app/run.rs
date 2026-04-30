use crate::{
    app::App, app::factory::AppFactory, cli, config::server::TlsConfig,
    error::init::InitError, error::runtime::RuntimeError,
};
use axum_server::{accept::NoDelayAcceptor, tls_rustls::RustlsConfig};
use futures::future::BoxFuture;
use meltdown::Token;
use std::net::SocketAddr;
use tracing::info;

impl meltdown::Service for App {
    type Future = BoxFuture<'static, Result<(), RuntimeError>>;

    fn run(self, token: Token) -> Self::Future {
        Box::pin(async move {
            let app_state = self.state.clone();
            let config = app_state.config();
            let addr =
                SocketAddr::from((config.server.address, config.server.port));
            info!(address = %addr, tls = %config.server.tls, "server starting");

            let handle = axum_server::Handle::new();
            let app_factory = AppFactory::new_hyper_app(self);

            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            let autodefault_id = crate::types::router::RouterId::Named(
                compact_str::CompactString::new("autodefault"),
            );
            let has_autodefault = config.routers.contains_key(&autodefault_id);
            cli::helpers::show_welcome_banner(&addr, has_autodefault);

            match &config.server.tls {
                TlsConfig::Enabled { cert, key } => {
                    let tls_config =
                        RustlsConfig::from_pem_file(cert.clone(), key.clone())
                            .await
                            .map_err(InitError::Tls)?;

                    tokio::select! {
                        biased;
                        server_output = axum_server::bind_rustls(addr, tls_config)
                            .acceptor(NoDelayAcceptor)
                            .handle(handle.clone())
                            .serve(app_factory) => server_output.map_err(RuntimeError::Serve)?,
                        () = token => { handle.graceful_shutdown(Some(config.server.shutdown_timeout)); }
                    };
                }
                TlsConfig::Disabled => {
                    tokio::select! {
                        biased;
                        server_output = axum_server::bind(addr)
                            .handle(handle.clone())
                            .serve(app_factory) => server_output.map_err(RuntimeError::Serve)?,
                        () = token => { handle.graceful_shutdown(Some(config.server.shutdown_timeout)); }
                    };
                }
            }
            Ok(())
        })
    }
}
