pub mod chat_completions;

use strum::EnumIter;

use super::EndpointType;
use crate::{
    endpoints::{
        Endpoint, openai::OpenAI, openrouter::chat_completions::ChatCompletions,
    },
    error::invalid_req::InvalidRequestError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter)]
pub enum OpenRouter {
    ChatCompletions(ChatCompletions),
}

impl OpenRouter {
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::ChatCompletions(_) => ChatCompletions::PATH,
        }
    }

    #[must_use]
    pub fn chat_completions() -> Self {
        Self::ChatCompletions(ChatCompletions)
    }

    #[must_use]
    pub fn endpoint_type(&self) -> EndpointType {
        match self {
            Self::ChatCompletions(_) => EndpointType::Chat,
        }
    }
}

impl TryFrom<&str> for OpenRouter {
    type Error = InvalidRequestError;

    fn try_from(path: &str) -> Result<Self, Self::Error> {
        match path {
            ChatCompletions::PATH => Ok(Self::ChatCompletions(ChatCompletions)),
            path => {
                tracing::debug!(path = %path, "unsupported openrouter path");
                Err(InvalidRequestError::NotFound(path.to_string()))
            }
        }
    }
}

impl From<OpenAI> for OpenRouter {
    fn from(openai: OpenAI) -> Self {
        match openai {
            OpenAI::ChatCompletions(_) => {
                Self::ChatCompletions(ChatCompletions)
            }
        }
    }
}
