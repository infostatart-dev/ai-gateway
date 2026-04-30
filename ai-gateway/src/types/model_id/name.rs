use super::ModelId;
use derive_more::AsRef;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq, Hash, AsRef, Serialize, Deserialize)]
pub struct ModelName<'a>(pub(crate) Cow<'a, str>);

impl<'a> ModelName<'a> {
    #[must_use]
    pub fn borrowed(name: &'a str) -> Self {
        Self(Cow::Borrowed(name))
    }

    #[must_use]
    pub fn owned(name: String) -> Self {
        Self(Cow::Owned(name))
    }

    #[must_use]
    pub fn from_model(model: &'a ModelId) -> Self {
        match model {
            ModelId::ModelIdWithVersion { id, .. } => {
                Self(Cow::Borrowed(id.model.as_str()))
            }
            ModelId::Bedrock(bedrock_model_id) => {
                Self(Cow::Borrowed(bedrock_model_id.model.as_str()))
            }
            ModelId::Ollama(ollama_model_id) => {
                Self(Cow::Borrowed(ollama_model_id.model.as_str()))
            }
            ModelId::Unknown(model_id) => {
                Self(Cow::Borrowed(model_id.as_str()))
            }
        }
    }
}

impl std::fmt::Display for ModelName<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
