use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::{
    Json, Router,
    extract::{ConnectInfo, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::json;

use crate::{
    profiles::ForcedProfile,
    state::{ProfileRequest, SharedState},
};

pub fn routes(state: SharedState) -> Router {
    Router::new()
        .route("/_admin/reset", post(reset))
        .route("/_admin/state", get(state_snapshot))
        .route("/_admin/profile", post(set_profile))
        .with_state(state)
}

async fn reset(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<SharedState>,
) -> Response {
    if !is_loopback(addr.ip()) {
        return forbidden();
    }
    state.limits.reset();
    state.profiles.reset();
    StatusCode::NO_CONTENT.into_response()
}

async fn state_snapshot(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<SharedState>,
) -> Response {
    if !is_loopback(addr.ip()) {
        return forbidden();
    }
    let buckets = state
        .limits
        .snapshot()
        .into_iter()
        .map(|(scope, rpm, tpm, rpd)| {
            json!({ "scope": scope, "rpm_used": rpm, "tpm_used": tpm, "rpd_used": rpd })
        })
        .collect::<Vec<_>>();
    Json(json!({ "buckets": buckets })).into_response()
}

async fn set_profile(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<SharedState>,
    Json(body): Json<ProfileRequest>,
) -> Response {
    if !is_loopback(addr.ip()) {
        return forbidden();
    }
    let profile = match body.action.as_str() {
        "force-auth-error" => ForcedProfile::AuthError,
        "quota-exhausted" => ForcedProfile::QuotaExhausted,
        "overload" => ForcedProfile::Overload,
        "not-found" | "not_found" | "404" => ForcedProfile::NotFound,
        "high-demand" | "high_demand" | "503-high-demand" => {
            ForcedProfile::HighDemand
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "unknown action" })),
            )
                .into_response();
        }
    };
    state.profiles.force(&body.scope, profile);
    StatusCode::NO_CONTENT.into_response()
}

fn is_loopback(ip: IpAddr) -> bool {
    matches!(ip, IpAddr::V4(v4) if v4 == Ipv4Addr::LOCALHOST)
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, "admin routes are loopback-only").into_response()
}
