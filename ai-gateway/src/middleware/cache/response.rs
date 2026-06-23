use std::time::SystemTime;

use http::{HeaderMap, HeaderValue, StatusCode};
use http_body_util::BodyExt;
use http_cache::CacheManager;
use http_cache_semantics::{CachePolicy, ResponseLike};

use super::{
    context::CacheContext,
    utils::{
        CACHE_BUCKET_IDX, CACHE_HIT_HEADER, CACHE_MISS_HEADER_VALUE,
        bucket_header_value, build_response, get_url, get_version,
        header_map_to_hash_map,
    },
};
use crate::{
    cache::CacheClient,
    error::{api::ApiError, internal::InternalError},
    types::{request::Request, response::Response},
};

pub async fn handle_response_for_cache_miss(
    cache: &CacheClient,
    ctx: &CacheContext,
    key: String,
    req: Request,
    resp: Response,
    bucket: u8,
    now: SystemTime,
) -> Result<Response, ApiError> {
    let cacheable_resp =
        CacheableResponse::new(ctx, resp.headers(), resp.status());
    let policy = CachePolicy::new_options(
        &req,
        &cacheable_resp,
        now,
        ctx.options.unwrap_or_default(),
    );

    if !policy.is_storable() || !resp.status().is_success() {
        return Ok(resp);
    }
    let url = get_url(&req)?;
    let (parts, body) = resp.into_parts();
    let body_bytes = body
        .collect()
        .await
        .map_err(InternalError::CollectBodyError)?
        .to_bytes();
    let http_resp = http_cache::HttpResponse {
        body: body_bytes.clone().into(),
        headers: header_map_to_hash_map(parts.headers),
        status: parts.status.as_u16(),
        url,
        version: get_version(parts.version),
    };

    let cached = cache
        .put(key, http_resp, policy)
        .await
        .map_err(InternalError::CacheError)?;
    build_response(
        cached,
        parts.status,
        vec![
            (CACHE_HIT_HEADER, CACHE_MISS_HEADER_VALUE),
            (CACHE_BUCKET_IDX, bucket_header_value(bucket)),
        ],
    )
    .map_err(Into::into)
}

pub struct CacheableResponse {
    resp_headers: HeaderMap,
    status: StatusCode,
}
impl CacheableResponse {
    pub fn new(
        ctx: &CacheContext,
        resp: &HeaderMap,
        status: StatusCode,
    ) -> Self {
        let mut resp_headers = resp.clone();
        resp_headers.remove(http::header::SET_COOKIE);
        resp_headers.remove(&super::utils::CACHE_HIT_HEADER);
        resp_headers.remove(&super::utils::CACHE_BUCKET_IDX);
        if let Some(directive) = ctx.directive.as_ref()
            && let Some(value) =
                cache_control::CacheControl::from_value(directive)
        {
            if let Some(max_age) = value.max_age {
                resp_headers.append(
                    http::header::CACHE_CONTROL,
                    HeaderValue::from_str(&format!(
                        "max-age={}",
                        max_age.as_secs()
                    ))
                    .unwrap(),
                );
            }
            if value.must_revalidate {
                resp_headers.append(
                    http::header::CACHE_CONTROL,
                    HeaderValue::from_static("must-revalidate"),
                );
            }
            if value.proxy_revalidate {
                resp_headers.append(
                    http::header::CACHE_CONTROL,
                    HeaderValue::from_static("proxy-revalidate"),
                );
            }
            if value.no_store {
                resp_headers.append(
                    http::header::CACHE_CONTROL,
                    HeaderValue::from_static("no-store"),
                );
            }
            if value.no_transform {
                resp_headers.append(
                    http::header::CACHE_CONTROL,
                    HeaderValue::from_static("no-transform"),
                );
            }
            match value.cachability {
                Some(cache_control::Cachability::Private) => {
                    resp_headers.append(
                        http::header::CACHE_CONTROL,
                        HeaderValue::from_static("private"),
                    );
                }
                Some(cache_control::Cachability::Public) => {
                    resp_headers.append(
                        http::header::CACHE_CONTROL,
                        HeaderValue::from_static("public"),
                    );
                }
                Some(cache_control::Cachability::NoCache) => {
                    resp_headers.append(
                        http::header::CACHE_CONTROL,
                        HeaderValue::from_static("no-cache"),
                    );
                }
                _ => {}
            }
        }
        Self {
            resp_headers,
            status,
        }
    }
}
impl ResponseLike for CacheableResponse {
    fn status(&self) -> StatusCode {
        self.status
    }
    fn headers(&self) -> &HeaderMap {
        &self.resp_headers
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use bytes::Bytes;
    use chrono::Utc;
    use http::{Method, StatusCode};
    use http_cache::MokaManager;

    use super::*;
    use crate::{
        app_state::AppState,
        cache::CacheClient,
        metrics::provider::usage_json::GATEWAY_PROVIDER_USAGE_HEADER,
        middleware::cache::{
            check::{CacheCheckResult, check_cache},
            context::CacheContext,
            utils::{CACHE_HIT_HEADER, CACHE_HIT_HEADER_VALUE},
        },
        types::body::Body,
    };

    fn cache_request(body: Bytes) -> Request {
        http::Request::builder()
            .method(Method::POST)
            .uri("http://localhost/router/my-router/chat/completions")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap()
    }

    fn usage_header_value() -> HeaderValue {
        HeaderValue::from_str(
            r#"{"provider":"openai","model":"gpt-4o-mini","usage":{"input":10,"output":5,"total":15,"source":"reported"},"latency_ms":{"total":100.0},"routing":{"attempts":1,"failover":false}}"#,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn cache_hit_returns_cached_x_gateway_provider_usage() {
        let app_state = AppState::test_default().await;
        let cache = CacheClient::Moka(MokaManager::new(
            moka::future::Cache::builder().build(),
        ));
        let ctx = CacheContext {
            enabled: Some(true),
            directive: Some("max-age=3600".to_string()),
            buckets: None,
            seed: None,
            options: None,
        };
        let body = Bytes::from_static(br#"{"model":"gpt-4o-mini"}"#);
        let usage = usage_header_value();
        let key = "usage-header-round-trip".to_string();
        let now = SystemTime::now();

        let miss_response = http::Response::builder()
            .status(StatusCode::OK)
            .header(GATEWAY_PROVIDER_USAGE_HEADER, usage.clone())
            .header("cache-control", "max-age=3600")
            .body(Body::from(Bytes::from_static(br#"{"cached":true}"#)))
            .unwrap();

        let miss = handle_response_for_cache_miss(
            &cache,
            &ctx,
            key.clone(),
            cache_request(body.clone()),
            miss_response,
            0,
            now,
        )
        .await
        .unwrap();

        assert_eq!(
            miss.headers().get(&CACHE_HIT_HEADER),
            Some(&CACHE_MISS_HEADER_VALUE)
        );
        assert_eq!(
            miss.headers().get(GATEWAY_PROVIDER_USAGE_HEADER),
            Some(&usage)
        );

        let mut hit_request = cache_request(body);
        hit_request
            .extensions_mut()
            .insert(tokio::time::Instant::now());
        hit_request.extensions_mut().insert(Utc::now());

        let hit =
            check_cache(app_state, &cache, &key, hit_request, 0, now, &ctx)
                .await
                .unwrap();

        let CacheCheckResult::Fresh(response) = hit else {
            panic!("expected fresh cache hit");
        };
        assert_eq!(
            response.headers().get(&CACHE_HIT_HEADER),
            Some(&CACHE_HIT_HEADER_VALUE)
        );
        assert_eq!(
            response.headers().get(GATEWAY_PROVIDER_USAGE_HEADER),
            Some(&usage),
            "cache hit must return the stored x-gateway-provider-usage header"
        );
    }
}
