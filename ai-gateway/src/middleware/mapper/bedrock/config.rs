use async_openai::types::chat as openai;

use crate::{
    endpoints::bedrock::converse::InferenceConfig,
    middleware::mapper::DEFAULT_MAX_TOKENS,
};

pub fn map_inference_config(
    max_completion_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    stop: Option<openai::StopConfiguration>,
) -> Option<InferenceConfig> {
    let stop_sequences = match stop {
        Some(openai::StopConfiguration::String(s)) => Some(vec![s]),
        Some(openai::StopConfiguration::StringArray(ss)) => Some(ss),
        _ => None,
    };

    Some(InferenceConfig {
        top_p,
        temperature,
        max_tokens: Some(
            i32::try_from(max_completion_tokens.unwrap_or(DEFAULT_MAX_TOKENS))
                .unwrap_or(DEFAULT_MAX_TOKENS as i32),
        ),
        stop_sequences,
    })
}
