use std::net::SocketAddr;

use axum::Router;
pub use catalog::ProviderTable;
pub use config::EmulatorConfig;
pub use state::SharedState;
pub use welcome::WELCOME;

mod admin;
mod capability;
mod catalog;
mod config;
mod engine;
mod family;
pub mod limits;
mod payload;
mod profiles;
mod router;
mod schema_fill;
mod state;
mod tier;
mod tokens;
mod welcome;
mod wire;

pub fn router(state: &SharedState) -> Router {
    router::build(state)
}

pub async fn bind_and_serve(config: EmulatorConfig) -> std::io::Result<()> {
    let addr = format!("{}:{}", config.address, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "upstream-emulator listening");
    let state = SharedState::new(config);
    axum::serve(
        listener,
        router(&state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
}

pub async fn bind_ephemeral()
-> std::io::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let state = SharedState::new(EmulatorConfig {
        port: addr.port(),
        default_latency_ms: 0,
        ..EmulatorConfig::default()
    });
    let handle = tokio::spawn(async move {
        axum::serve(
            listener,
            router(&state).into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("emulator serve");
    });
    Ok((addr, handle))
}
