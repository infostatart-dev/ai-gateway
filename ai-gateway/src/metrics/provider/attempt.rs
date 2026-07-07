use http::StatusCode;

use crate::metrics::llm::TokenUsage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageSource {
    Reported,
    Estimated,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallOutcome {
    Success,
    SuccessDegraded,
    SemanticError,
    ClientError,
    ServerError,
    RateLimited,
    Overload,
}

impl CallOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::SuccessDegraded => "success_degraded",
            Self::SemanticError => "semantic_error",
            Self::ClientError => "client_error",
            Self::ServerError => "server_error",
            Self::RateLimited => "rate_limited",
            Self::Overload => "overload",
        }
    }
}

#[must_use]
pub fn classify_outcome(
    status: StatusCode,
    usage_source: UsageSource,
    overload: bool,
) -> CallOutcome {
    if status == StatusCode::TOO_MANY_REQUESTS {
        return CallOutcome::RateLimited;
    }
    if overload && status == StatusCode::SERVICE_UNAVAILABLE {
        return CallOutcome::Overload;
    }
    if status.is_server_error() {
        return CallOutcome::ServerError;
    }
    if status.is_client_error() {
        return CallOutcome::ClientError;
    }
    if status.is_success() {
        return if usage_source == UsageSource::Reported {
            CallOutcome::Success
        } else {
            CallOutcome::SuccessDegraded
        };
    }
    CallOutcome::ClientError
}

#[must_use]
pub fn resolve_usage(
    reported: TokenUsage,
    estimate_input: Option<u64>,
    estimate_output: Option<u64>,
    estimate_enabled: bool,
) -> (TokenUsage, UsageSource) {
    if !reported.is_empty() {
        return (reported, UsageSource::Reported);
    }
    if !estimate_enabled {
        return (TokenUsage::default(), UsageSource::None);
    }
    let mut usage = TokenUsage {
        input: estimate_input,
        output: estimate_output,
        ..TokenUsage::default()
    };
    if usage.input.is_some() || usage.output.is_some() {
        usage.total = Some(
            usage
                .input
                .unwrap_or(0)
                .saturating_add(usage.output.unwrap_or(0)),
        );
        (usage, UsageSource::Estimated)
    } else {
        (TokenUsage::default(), UsageSource::None)
    }
}
