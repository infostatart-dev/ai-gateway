use std::sync::{Arc, Mutex, OnceLock};

use crate::tls::client::shared_client;
use crate::Error;

#[derive(Debug, Clone)]
pub struct FetchRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone)]
pub struct FetchResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl FetchResponse {
    pub fn header(&self, name: &str) -> Option<String> {
        let lower = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_ascii_lowercase() == lower)
            .map(|(_, v)| v.clone())
    }
}

pub trait HttpFetch: Send + Sync {
    fn fetch<'a>(
        &'a self,
        req: FetchRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<FetchResponse, Error>> + Send + 'a>,
    >;
}

pub struct RquestFetch;

impl HttpFetch for RquestFetch {
    fn fetch<'a>(
        &'a self,
        req: FetchRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<FetchResponse, Error>> + Send + 'a>,
    > {
        Box::pin(async move {
            let client = shared_client()?;
            let method = req
                .method
                .parse::<wreq::Method>()
                .map_err(|e| Error::Other(format!("{e}")))?;
            let mut builder = client.request(method, &req.url);
            for (k, v) in req.headers {
                builder = builder.header(k, v);
            }
            if let Some(body) = req.body {
                builder = builder.body(body);
            }
            let resp = builder
                .send()
                .await
                .map_err(|e| Error::Tls(e.to_string()))?;
            let status = resp.status().as_u16();
            let mut headers = Vec::new();
            for (k, v) in resp.headers().iter() {
                headers.push((
                    k.to_string(),
                    v.to_str().unwrap_or_default().to_string(),
                ));
            }
            let body = resp
                .bytes()
                .await
                .map_err(|e| Error::Tls(e.to_string()))?
                .to_vec();
            Ok(FetchResponse {
                status,
                headers,
                body,
            })
        })
    }
}

static FETCH_OVERRIDE: OnceLock<Arc<dyn HttpFetch>> = OnceLock::new();

pub fn set_fetch_override(fetch: Arc<dyn HttpFetch>) {
    let _ = FETCH_OVERRIDE.set(fetch);
}

pub fn default_fetch() -> Arc<dyn HttpFetch> {
    FETCH_OVERRIDE
        .get()
        .cloned()
        .unwrap_or_else(|| Arc::new(RquestFetch))
}

#[derive(Default)]
pub struct MockFetch {
    responses: Mutex<Vec<FetchResponse>>,
    calls: Mutex<usize>,
}

impl MockFetch {
    pub fn new(responses: Vec<FetchResponse>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(responses),
            calls: Mutex::new(0),
        })
    }

    pub fn call_count(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

impl HttpFetch for MockFetch {
    fn fetch<'a>(
        &'a self,
        _req: FetchRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<FetchResponse, Error>> + Send + 'a>,
    > {
        Box::pin(async move {
            let mut calls = self.calls.lock().unwrap();
            *calls += 1;
            let idx = *calls - 1;
            let responses = self.responses.lock().unwrap();
            responses
                .get(idx)
                .cloned()
                .ok_or_else(|| Error::Other("mock fetch exhausted".into()))
        })
    }
}
