use super::{
    check::{CacheCheckResult, check_cache},
    context::CacheContext,
    response::handle_response_for_cache_miss,
    utils::{get_hasher, record_cache_hit, record_cache_miss},
};
use crate::{
    app_state::AppState,
    cache::CacheClient,
    config::cache::DEFAULT_BUCKETS,
    error::{api::ApiError, internal::InternalError},
    types::{request::Request, response::Response},
};
use futures::{StreamExt, stream::FuturesUnordered};
use http::HeaderValue;
use http_body_util::BodyExt;
use std::convert::Infallible;
use std::hash::Hash;

pub async fn make_request<S>(
    inner: &mut S,
    app_state: &AppState,
    mut req: Request,
    cache: &CacheClient,
    ctx: CacheContext,
) -> Result<Response, ApiError>
where
    S: tower::Service<Request, Response = Response, Error = Infallible>
        + Send
        + 'static,
{
    if ctx.enabled.is_none_or(|enabled| !enabled) {
        return inner
            .call(req)
            .await
            .map_err(|_| ApiError::Internal(InternalError::Internal));
    }

    if let Some(directive) = &ctx.directive {
        if req.headers().get(http::header::CACHE_CONTROL).is_none() {
            req.headers_mut().insert(
                http::header::CACHE_CONTROL,
                HeaderValue::from_str(directive)
                    .map_err(InternalError::InvalidHeader)?,
            );
        }
    }

    let (parts, body) = req.into_parts();
    let body_bytes = body
        .collect()
        .await
        .map_err(InternalError::CollectBodyError)?
        .to_bytes();
    let buckets = ctx.buckets.unwrap_or(DEFAULT_BUCKETS).max(1);
    let now = std::time::SystemTime::now();

    let mut futures = FuturesUnordered::new();
    let hasher = get_hasher(&parts, &body_bytes, ctx.seed.as_deref());
    let mut bucket_indices: Vec<u8> = (0..buckets).collect();
    {
        use rand::seq::SliceRandom;
        let mut rng = rand::rng();
        bucket_indices.shuffle(&mut rng);
    }

    for bucket in bucket_indices {
        let mut cloned_hasher = hasher.clone();
        bucket.hash(&mut cloned_hasher);
        let key = std::hash::Hasher::finish(&cloned_hasher).to_string();
        let req = Request::from_parts(parts.clone(), body_bytes.clone().into());
        let ctx_ref = &ctx;
        futures.push(async move {
            check_cache(
                app_state.clone(),
                cache,
                &key,
                req,
                bucket,
                now,
                ctx_ref,
            )
            .await
            .map(|result| (bucket, key, result))
        });
    }

    let mut stale_hits = Vec::new();
    let mut empty_buckets = Vec::new();

    while let Some(result) = futures.next().await {
        match result {
            Ok((bucket, _key, CacheCheckResult::Fresh(mut resp))) => {
                record_cache_hit(app_state, bucket, &parts.uri);
                resp.headers_mut().extend([
                    (
                        super::utils::CACHE_HIT_HEADER,
                        super::utils::CACHE_HIT_HEADER_VALUE,
                    ),
                    (
                        super::utils::CACHE_BUCKET_IDX,
                        super::utils::bucket_header_value(bucket),
                    ),
                ]);
                return Ok(resp);
            }
            Ok((bucket, key, CacheCheckResult::Stale)) => {
                stale_hits.push((bucket, key))
            }
            Ok((bucket, _, CacheCheckResult::Miss)) => {
                empty_buckets.push(bucket)
            }
            Err(e) => tracing::warn!(error = %e, "Cache check error"),
        }
    }

    if let Some((bucket, key)) = stale_hits.into_iter().next() {
        let resp = inner
            .call(Request::from_parts(
                parts.clone(),
                body_bytes.clone().into(),
            ))
            .await
            .map_err(|_| ApiError::Internal(InternalError::Internal))?;
        return handle_response_for_cache_miss(
            cache,
            &ctx,
            key,
            Request::from_parts(parts, body_bytes.into()),
            resp,
            bucket,
            now,
        )
        .await;
    }

    let bucket = empty_buckets
        .first()
        .copied()
        .unwrap_or_else(|| rand::random::<u8>() % buckets);
    let mut cloned_hasher = hasher.clone();
    bucket.hash(&mut cloned_hasher);
    let key = std::hash::Hasher::finish(&cloned_hasher).to_string();
    record_cache_miss(app_state, &parts.uri, bucket);

    let resp = inner
        .call(Request::from_parts(
            parts.clone(),
            body_bytes.clone().into(),
        ))
        .await
        .map_err(|_| ApiError::Internal(InternalError::Internal))?;
    handle_response_for_cache_miss(
        cache,
        &ctx,
        key,
        Request::from_parts(parts, body_bytes.into()),
        resp,
        bucket,
        now,
    )
    .await
}
