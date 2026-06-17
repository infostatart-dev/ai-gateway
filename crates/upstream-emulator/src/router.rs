use axum::{Router, extract::Request, response::IntoResponse, routing::any};

use crate::{
    catalog::ProviderEntry, engine::dispatch_api_key, state::SharedState,
};

pub fn build(state: &SharedState) -> Router {
    let mut router = Router::new().merge(crate::admin::routes(state.clone()));
    for entry in state.table.entries.clone() {
        let mount = format!("/{}", entry.id);
        router = router.nest(&mount, provider_router(state.clone(), entry));
    }
    router.route("/health", axum::routing::get(|| async { "ok" }))
}

fn provider_router(state: SharedState, entry: ProviderEntry) -> Router {
    Router::new().fallback(any(move |req: Request| {
        let state = state.clone();
        let entry = entry.clone();
        async move { handle(state, entry, req).await }
    }))
}

async fn handle(
    state: SharedState,
    entry: ProviderEntry,
    req: Request,
) -> impl IntoResponse {
    let (parts, body) = req.into_parts();
    let method = parts.method;
    let headers = parts.headers;
    let body = axum::body::to_bytes(body, usize::MAX)
        .await
        .unwrap_or_default();
    dispatch_api_key(
        axum::extract::State(state),
        entry.id,
        entry.family,
        method,
        headers,
        body,
    )
    .await
}
