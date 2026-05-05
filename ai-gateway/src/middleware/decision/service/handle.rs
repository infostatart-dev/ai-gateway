use tower::Service;

use super::{prepare, resolve, wrap};
use crate::{
    app_state::AppState,
    error::api::ApiError,
    middleware::decision::policy::{KeyPolicy, Tier},
    types::{extensions::AuthContext, request::Request, response::Response},
};

/// Header через который агент может выбрать тир запроса. Перебивает
/// `policy.tier`, разрешённый policy_store, для конкретного запроса.
/// Допустимые значения: `free`, `freemium`, `paid` (case-insensitive).
const TIER_OVERRIDE_HEADER: &str = "x-decision-tier";

fn parse_tier_override(req: &Request) -> Option<Tier> {
    let raw = req.headers().get(TIER_OVERRIDE_HEADER)?.to_str().ok()?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "free" => Some(Tier::Free),
        "freemium" => Some(Tier::Freemium),
        "paid" => Some(Tier::Paid),
        _ => None,
    }
}

pub(super) async fn handle_decision_request<S>(
    inner: &mut S,
    app_state: AppState,
    req: Request,
) -> Result<Response, ApiError>
where
    S: Service<Request, Response = Response, Error = ApiError>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    let auth = req.extensions().get::<AuthContext>().cloned();
    let existing_policy = req.extensions().get::<KeyPolicy>().cloned();
    let mut policy =
        resolve::resolve_policy(&app_state, existing_policy, auth.as_ref())
            .await?;

    // Per-request tier override через заголовок X-Decision-Tier.
    // Полезно для агентов, которые сами знают какой intent шлют, и не хотят
    // тащить per-key policy в gateway-конфиг.
    if let Some(override_tier) = parse_tier_override(&req) {
        if override_tier != policy.tier {
            tracing::info!(
                from = ?policy.tier,
                to = ?override_tier,
                "decision tier overridden by X-Decision-Tier header",
            );
            policy.tier = override_tier;
        }
    }

    let permit = resolve::acquire_traffic_slot(&app_state, &policy).await?;
    let prepared = prepare::prepare_request(req, &policy).await?;
    let state_store = app_state.0.state_store.clone();
    let budget_key = policy.budget_namespace.clone();
    let reservation_id = state_store
        .reserve(&budget_key, prepared.reserved_output_tokens)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "budget reservation failed");
            ApiError::Internal(
                crate::error::internal::InternalError::DecisionEngineError(
                    error,
                ),
            )
        })?;

    match inner.call(prepared.request).await {
        Ok(response) => Ok(wrap::wrap_response_body(
            response,
            state_store,
            budget_key,
            reservation_id,
            prepared.reserved_output_tokens,
            permit,
        )),
        Err(error) => {
            let _ = state_store
                .refund_reservation(&budget_key, &reservation_id)
                .await;
            drop(permit);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tier_override_tests {
    use axum_core::body::Body;
    use http::Request as HttpRequest;

    use super::{parse_tier_override, Tier};

    fn req(header_value: Option<&str>) -> HttpRequest<Body> {
        let mut builder = HttpRequest::builder();
        if let Some(v) = header_value {
            builder = builder.header("x-decision-tier", v);
        }
        builder.body(Body::empty()).unwrap()
    }

    #[test]
    fn parses_free() {
        assert_eq!(parse_tier_override(&req(Some("free"))), Some(Tier::Free));
    }

    #[test]
    fn parses_freemium() {
        assert_eq!(
            parse_tier_override(&req(Some("freemium"))),
            Some(Tier::Freemium)
        );
    }

    #[test]
    fn parses_paid_case_insensitive() {
        assert_eq!(parse_tier_override(&req(Some("PAID"))), Some(Tier::Paid));
        assert_eq!(parse_tier_override(&req(Some("Paid"))), Some(Tier::Paid));
    }

    #[test]
    fn missing_header_returns_none() {
        assert_eq!(parse_tier_override(&req(None)), None);
    }

    #[test]
    fn unknown_value_returns_none() {
        assert_eq!(parse_tier_override(&req(Some("premium"))), None);
        assert_eq!(parse_tier_override(&req(Some(""))), None);
    }
}
