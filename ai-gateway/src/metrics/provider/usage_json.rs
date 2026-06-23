use http::HeaderValue;
use serde::Serialize;

pub const GATEWAY_PROVIDER_USAGE_HEADER: &str = "x-gateway-provider-usage";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GatewayProviderUsage {
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub usage: UsageBlock,
    pub latency_ms: LatencyBlock,
    pub routing: RoutingBlock,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UsageBlock {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    pub source: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LatencyBlock {
    pub total: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttft: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_per_output_token: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RoutingBlock {
    pub attempts: u32,
    pub failover: bool,
}

impl GatewayProviderUsage {
    #[must_use]
    pub fn to_header_value(&self) -> Option<HeaderValue> {
        let json = serde_json::to_string(self).ok()?;
        if json.len() > 4096 {
            return None;
        }
        HeaderValue::from_str(&json).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_usage() -> GatewayProviderUsage {
        GatewayProviderUsage {
            provider: "openai".to_string(),
            credential: None,
            model: Some("gpt-4o-mini".to_string()),
            usage: UsageBlock {
                input: Some(19),
                output: Some(10),
                cached: None,
                reasoning: None,
                total: Some(29),
                source: "reported",
            },
            latency_ms: LatencyBlock {
                total: 120.0,
                ttft: None,
                generation_per_output_token: Some(8.5),
            },
            routing: RoutingBlock {
                attempts: 1,
                failover: false,
            },
        }
    }

    #[test]
    fn header_json_parses_and_stays_under_limit() {
        let usage = sample_usage();
        let header = usage.to_header_value().expect("valid header");
        let parsed: serde_json::Value =
            serde_json::from_str(header.to_str().unwrap()).unwrap();
        assert_eq!(parsed["provider"], "openai");
        assert_eq!(parsed["usage"]["source"], "reported");
        assert!(header.to_str().unwrap().len() <= 4096);
    }

    #[test]
    fn oversized_payload_omits_header() {
        let mut usage = sample_usage();
        usage.model = Some("x".repeat(5000));
        assert!(usage.to_header_value().is_none());
    }
}
