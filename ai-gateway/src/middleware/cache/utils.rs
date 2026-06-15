use std::{collections::HashMap, hash::Hash};

use http::{HeaderMap, HeaderName, HeaderValue, StatusCode, request::Parts};
use http_cache::HttpResponse;
use opentelemetry::KeyValue;
use rustc_hash::FxHasher;
use url::Url;

use crate::{
    app_state::AppState,
    error::internal::InternalError,
    types::{response::Response, router::RouterId},
};

pub const CACHE_HIT_HEADER: HeaderName =
    HeaderName::from_static("helicone-cache");
pub const CACHE_BUCKET_IDX: HeaderName =
    HeaderName::from_static("helicone-cache-bucket-idx");
pub const CACHE_HIT_HEADER_VALUE: HeaderValue = HeaderValue::from_static("HIT");
pub const CACHE_MISS_HEADER_VALUE: HeaderValue =
    HeaderValue::from_static("MISS");

#[must_use]
pub fn bucket_header_value(bucket: u8) -> HeaderValue {
    HeaderValue::from_str(&bucket.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static("0"))
}

pub fn build_response(
    cached: HttpResponse,
    status: StatusCode,
    extra_headers: impl IntoIterator<Item = (HeaderName, HeaderValue)>,
) -> Result<Response, InternalError> {
    let mut builder = http::Response::builder().status(status);
    for (k, v) in cached.headers {
        builder = builder.header(k, v);
    }
    let mut response = builder
        .body(cached.body.into())
        .map_err(|_| InternalError::Internal)?;
    response.headers_mut().extend(extra_headers);
    Ok(response)
}

pub fn get_url(
    req: &http::Request<crate::types::body::Body>,
) -> Result<Url, crate::error::invalid_req::InvalidRequestError> {
    let host = req.uri().host().unwrap_or("localhost");
    let scheme = req.uri().scheme().unwrap_or(&http::uri::Scheme::HTTP);
    let full_url = format!("{}://{}{}", scheme, host, req.uri());
    Url::parse(&full_url).map_err(|e| {
        crate::error::invalid_req::InvalidRequestError::InvalidUrl(
            e.to_string(),
        )
    })
}

pub fn get_hasher(
    parts: &Parts,
    body: &bytes::Bytes,
    seed: Option<&str>,
    router_id: Option<&RouterId>,
) -> FxHasher {
    let mut hasher = FxHasher::default();
    if let Some(s) = seed {
        s.hash(&mut hasher);
    }
    if let Some(router_id) = router_id {
        router_id.to_string().hash(&mut hasher);
    }
    if let Some(pq) = parts.uri.path_and_query() {
        pq.hash(&mut hasher);
    }
    body.hash(&mut hasher);
    hasher
}

pub fn record_cache_hit(app_state: &AppState, bucket: u8, uri: &http::Uri) {
    let attrs = &[
        KeyValue::new("bucket", bucket.to_string()),
        KeyValue::new("path", uri.path().to_string()),
    ];
    app_state.0.metrics.cache.hits.add(1, attrs);
}

pub fn record_cache_miss(app_state: &AppState, uri: &http::Uri, bucket: u8) {
    let attrs = &[
        KeyValue::new("bucket", bucket.to_string()),
        KeyValue::new("path", uri.path().to_string()),
    ];
    app_state.0.metrics.cache.misses.add(1, attrs);
}

#[must_use]
pub fn get_version(version: http::Version) -> http_cache::HttpVersion {
    match version {
        http::Version::HTTP_09 => http_cache::HttpVersion::Http09,
        http::Version::HTTP_10 => http_cache::HttpVersion::Http10,
        http::Version::HTTP_2 => http_cache::HttpVersion::H2,
        http::Version::HTTP_3 => http_cache::HttpVersion::H3,
        _ => http_cache::HttpVersion::Http11,
    }
}

#[must_use]
pub fn header_map_to_hash_map(headers: HeaderMap) -> HashMap<String, String> {
    headers
        .into_iter()
        .filter_map(|(name, value)| {
            Some((name?.to_string(), value.to_str().ok()?.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::hash::Hasher;

    use compact_str::CompactString;
    use http::Request;

    use super::*;
    use crate::types::router::RouterId;

    fn cache_key(parts: &Parts, body: &bytes::Bytes) -> u64 {
        let router_id = parts.extensions.get::<RouterId>();
        let hasher = get_hasher(parts, body, None, router_id);
        hasher.finish()
    }

    #[test]
    fn cache_key_includes_router_id() {
        let body = bytes::Bytes::from_static(br#"{"model":"gpt-4"}"#);
        let mut req_a = Request::builder()
            .uri("http://localhost/chat/completions")
            .body(())
            .unwrap();
        req_a
            .extensions_mut()
            .insert(RouterId::Named(CompactString::from("autodefault")));
        let parts_a = req_a.into_parts().0;

        let mut req_b = Request::builder()
            .uri("http://localhost/chat/completions")
            .body(())
            .unwrap();
        req_b
            .extensions_mut()
            .insert(RouterId::Named(CompactString::from("other-router")));
        let parts_b = req_b.into_parts().0;

        assert_ne!(
            cache_key(&parts_a, &body),
            cache_key(&parts_b, &body),
            "same path/body on different routers must not share cache entries"
        );
    }
}
