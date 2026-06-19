use super::ResponseFactory;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HopTarget {
    pub credential_id: String,
    pub model: Option<String>,
}

impl HopTarget {
    #[must_use]
    pub fn binding(
        credential_id: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            credential_id: credential_id.into(),
            model: Some(model.into()),
        }
    }

    #[must_use]
    pub fn credential(credential_id: impl Into<String>) -> Self {
        Self {
            credential_id: credential_id.into(),
            model: None,
        }
    }
}

#[derive(Debug, Clone)]
struct Rule {
    target: HopTarget,
    sequence: Vec<ResponseFactory>,
    repeat_last: bool,
}

/// Declarative upstream response script — match by `(credential, model)`, not
/// FIFO order.
#[derive(Debug, Default)]
pub struct UpstreamMockScript {
    rules: Vec<Rule>,
    default: Option<ResponseFactory>,
}

impl UpstreamMockScript {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn binding(
        mut self,
        credential_id: impl Into<String>,
        model: impl Into<String>,
        sequence: Vec<ResponseFactory>,
    ) -> Self {
        self.rules.push(Rule {
            target: HopTarget::binding(credential_id, model),
            sequence,
            repeat_last: true,
        });
        self
    }

    #[must_use]
    pub fn credential(
        mut self,
        credential_id: impl Into<String>,
        sequence: Vec<ResponseFactory>,
    ) -> Self {
        self.rules.push(Rule {
            target: HopTarget::credential(credential_id),
            sequence,
            repeat_last: true,
        });
        self
    }

    #[must_use]
    pub fn default_response(mut self, factory: ResponseFactory) -> Self {
        self.default = Some(factory);
        self
    }

    pub(crate) fn resolve(
        &self,
        credential_id: &str,
        model: &str,
        attempt: u32,
    ) -> Option<ResponseFactory> {
        let rule = self
            .rules
            .iter()
            .find(|rule| rule.target.matches(credential_id, model))?;
        Some(rule.pick(attempt))
    }

    pub(crate) fn default_factory(&self) -> Option<ResponseFactory> {
        self.default
    }
}

impl HopTarget {
    fn matches(&self, credential_id: &str, model: &str) -> bool {
        if self.credential_id != credential_id {
            return false;
        }
        self.model
            .as_deref()
            .is_none_or(|expected| expected == model)
    }
}

impl Rule {
    fn pick(&self, attempt: u32) -> ResponseFactory {
        let index = attempt as usize;
        if index < self.sequence.len() {
            return self.sequence[index];
        }
        if self.repeat_last {
            return *self.sequence.last().expect("rule sequence is non-empty");
        }
        *self.sequence.last().expect("rule sequence is non-empty")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upstream::responses::{ok_chat_completion, rate_limited_rpm};

    #[test]
    fn binding_sequence_advances_per_hop() {
        let script = UpstreamMockScript::new().binding(
            "gemini-free-9",
            "gemini-3.1-flash-lite",
            vec![rate_limited_rpm, ok_chat_completion],
        );
        assert_eq!(
            script
                .resolve("gemini-free-9", "gemini-3.1-flash-lite", 0)
                .expect("first")()
            .status(),
            http::StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            script
                .resolve("gemini-free-9", "gemini-3.1-flash-lite", 1)
                .expect("second")()
            .status(),
            http::StatusCode::OK
        );
        assert!(script.resolve("gemini-free-9", "other-model", 0).is_none());
    }
}
