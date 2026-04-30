use http_cache_semantics::CacheOptions;
use crate::{
    error::invalid_req::InvalidRequestError,
    types::request::Request,
    config::cache::MAX_BUCKET_SIZE,
};

#[derive(Debug, Clone)]
pub struct CacheContext {
    pub enabled: Option<bool>,
    pub directive: Option<String>,
    pub buckets: Option<u8>,
    pub seed: Option<String>,
    pub options: Option<CacheOptions>,
}

impl CacheContext {
    pub fn merge(&self, other: &Self) -> Self {
        let enabled = other.enabled.or(self.enabled).unwrap_or(false);
        Self {
            enabled: Some(enabled),
            directive: other.directive.clone().or_else(|| self.directive.clone()),
            buckets: other.buckets.or(self.buckets),
            seed: other.seed.clone().or_else(|| self.seed.clone()),
            options: other.options.or(self.options),
        }
    }
}

pub fn get_cache_ctx(req: &Request) -> Result<CacheContext, InvalidRequestError> {
    let headers = req.headers();
    let enabled = headers.get("helicone-cache-enabled").and_then(|v| v.to_str().ok()?.parse::<bool>().ok());
    let buckets = headers.get("helicone-cache-bucket-max-size").and_then(|v| v.to_str().ok()?.parse::<u8>().ok());
    if buckets.is_some_and(|b| b > MAX_BUCKET_SIZE) { return Err(InvalidRequestError::InvalidCacheConfig); }
    let seed = headers.get("helicone-cache-seed").and_then(|v| v.to_str().ok().map(String::from));
    let directive = headers.get(http::header::CACHE_CONTROL).and_then(|v| v.to_str().ok().map(String::from));
    Ok(CacheContext { enabled, directive, buckets, seed, options: None })
}
