use std::time::SystemTime;

use chrono::{DateTime, Utc};
use http_cache::CacheManager;
use http_cache_semantics::BeforeRequest;

use super::{
    context::CacheContext,
    logging::spawn_cache_logging,
    utils::{bucket_header_value, build_response, get_url},
};
use crate::{
    app_state::AppState,
    cache::CacheClient,
    error::{api::ApiError, internal::InternalError},
    types::{body::BodyReader, request::Request, response::Response},
};

pub enum CacheCheckResult {
    Fresh(Response),
    Stale,
    Miss,
}

#[allow(clippy::too_many_lines)]
pub async fn check_cache(
    app_state: AppState,
    cache: &CacheClient,
    key: &str,
    req: Request,
    bucket: u8,
    now: SystemTime,
    ctx: &CacheContext,
) -> Result<CacheCheckResult, ApiError> {
    let Some((http_resp, policy)) =
        cache.get(key).await.map_err(InternalError::CacheError)?
    else {
        return Ok(CacheCheckResult::Miss);
    };

    match policy.before_request(&req, now) {
        BeforeRequest::Fresh(parts) => {
            let additional_headers = vec![
                (
                    super::utils::CACHE_HIT_HEADER,
                    super::utils::CACHE_HIT_HEADER_VALUE,
                ),
                (super::utils::CACHE_BUCKET_IDX, bucket_header_value(bucket)),
            ];
            let response =
                build_response(http_resp, parts.status, additional_headers)?;
            let start_instant = req
                .extensions()
                .get::<tokio::time::Instant>()
                .copied()
                .ok_or(InternalError::ExtensionNotFound("Instant"))?;
            let start_time =
                req.extensions()
                    .get::<DateTime<Utc>>()
                    .copied()
                    .ok_or(InternalError::ExtensionNotFound("DateTime<Utc>"))?;
            let target_url = get_url(&req)?;
            let req_headers = req.headers().clone();
            let (req_parts, req_body) = req.into_parts();

            let (resp_parts, resp_body) = response.into_parts();
            let (user_resp_body, body_reader, tfft_rx) =
                BodyReader::wrap_stream(
                    futures::TryStreamExt::map_err(
                        resp_body.into_data_stream(),
                        |e| InternalError::CollectBodyError(e).into(),
                    ),
                    false,
                );
            let response = Response::from_parts(resp_parts, user_resp_body);

            spawn_cache_logging(
                app_state,
                req_parts,
                req_body,
                body_reader,
                tfft_rx,
                start_time,
                start_instant,
                target_url,
                req_headers,
                parts.status,
                ctx,
                response.headers(),
            );
            Ok(CacheCheckResult::Fresh(response))
        }
        BeforeRequest::Stale { matches, .. } if matches => {
            Ok(CacheCheckResult::Stale)
        }
        BeforeRequest::Stale { .. } => Ok(CacheCheckResult::Miss),
    }
}
