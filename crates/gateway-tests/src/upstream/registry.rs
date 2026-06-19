use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use axum_core::body::Body;
use http::Response;

use super::script::UpstreamMockScript;

struct Registry {
    script: Option<UpstreamMockScript>,
    hop_attempts: HashMap<(String, String), u32>,
}

impl Registry {
    fn new() -> Self {
        Self {
            script: None,
            hop_attempts: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        self.script = None;
        self.hop_attempts.clear();
    }

    fn pop(
        &mut self,
        credential_id: &str,
        model: &str,
    ) -> Option<Response<Body>> {
        let script = self.script.as_ref()?;
        let key = (credential_id.to_string(), model.to_string());
        let attempt = *self.hop_attempts.entry(key.clone()).or_insert(0);
        let factory = script
            .resolve(credential_id, model, attempt)
            .or_else(|| script.default_factory())?;
        self.hop_attempts.insert(key, attempt.saturating_add(1));
        Some(factory())
    }
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::new()))
}

pub fn install_upstream_mock(script: UpstreamMockScript) {
    let mut reg = registry().lock().expect("gateway-tests registry");
    reg.script = Some(script);
    reg.hop_attempts.clear();
}

pub fn clear_upstream_mocks() {
    registry().lock().expect("gateway-tests registry").clear();
}

#[must_use]
pub fn pop_upstream_response(
    credential_id: &str,
    model: &str,
) -> Option<Response<Body>> {
    registry()
        .lock()
        .expect("gateway-tests registry")
        .pop(credential_id, model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upstream::responses::{ok_chat_completion, rate_limited_rpm};

    #[test]
    fn install_and_pop_binding_sequence() {
        clear_upstream_mocks();
        install_upstream_mock(UpstreamMockScript::new().binding(
            "gemini-free-9",
            "gemini-3.1-flash-lite",
            vec![rate_limited_rpm, ok_chat_completion],
        ));
        let first =
            pop_upstream_response("gemini-free-9", "gemini-3.1-flash-lite")
                .expect("first");
        assert_eq!(first.status(), http::StatusCode::TOO_MANY_REQUESTS);
        let second =
            pop_upstream_response("gemini-free-9", "gemini-3.1-flash-lite")
                .expect("second");
        assert_eq!(second.status(), http::StatusCode::OK);
    }
}
