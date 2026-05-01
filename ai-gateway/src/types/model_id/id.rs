use std::{
    fmt::{self, Display},
    str::FromStr,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    base::ModelIdWithVersion, bedrock::BedrockModelId, name::ModelName,
    ollama::OllamaModelId, version::Version,
};
use crate::{error::mapper::MapperError, types::provider::InferenceProvider};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModelId {
    ModelIdWithVersion {
        provider: InferenceProvider,
        id: ModelIdWithVersion,
    },
    Bedrock(BedrockModelId),
    Ollama(OllamaModelId),
    Unknown(String),
}

impl ModelId {
    pub(crate) fn from_str_and_provider(
        request_style: InferenceProvider,
        s: &str,
    ) -> Result<Self, MapperError> {
        match request_style {
            provider @ (InferenceProvider::OpenAI
            | InferenceProvider::Anthropic
            | InferenceProvider::GoogleGemini
            | InferenceProvider::OpenRouter) => {
                Ok(ModelId::ModelIdWithVersion {
                    id: ModelIdWithVersion::from_str(strip_provider_prefix(
                        &provider, s,
                    ))?,
                    provider,
                })
            }
            provider @ InferenceProvider::Named(_) => {
                Ok(ModelId::ModelIdWithVersion {
                    provider,
                    id: ModelIdWithVersion::from_str(s)?,
                })
            }
            InferenceProvider::Bedrock => {
                Ok(ModelId::Bedrock(BedrockModelId::from_str(s)?))
            }
            InferenceProvider::Ollama => {
                Ok(ModelId::Ollama(OllamaModelId::from_str(s)?))
            }
        }
    }

    #[must_use]
    pub fn inference_provider(&self) -> Option<InferenceProvider> {
        match self {
            ModelId::ModelIdWithVersion { provider, .. } => {
                Some(provider.clone())
            }
            ModelId::Bedrock(_) => Some(InferenceProvider::Bedrock),
            ModelId::Ollama(_) => Some(InferenceProvider::Ollama),
            ModelId::Unknown(_) => None,
        }
    }

    #[must_use]
    pub fn as_model_name(&self) -> ModelName<'_> {
        ModelName::from_model(self)
    }

    #[must_use]
    pub fn as_model_name_owned(&self) -> ModelName<'static> {
        match self {
            ModelId::ModelIdWithVersion { id, .. } => {
                ModelName::owned(id.model.clone())
            }
            ModelId::Bedrock(m) => ModelName::owned(m.model.clone()),
            ModelId::Ollama(m) => ModelName::owned(m.model.clone()),
            ModelId::Unknown(m) => ModelName::owned(m.clone()),
        }
    }

    #[must_use]
    pub fn with_latest_version(self) -> ModelId {
        match self {
            ModelId::ModelIdWithVersion { provider, id } => {
                ModelId::ModelIdWithVersion {
                    provider,
                    id: ModelIdWithVersion {
                        model: id.model,
                        version: Version::Latest,
                    },
                }
            }
            ModelId::Bedrock(m) => ModelId::Bedrock(BedrockModelId {
                version: Version::Latest,
                ..m
            }),
            ModelId::Ollama(_) | ModelId::Unknown(_) => self,
        }
    }
}

fn strip_provider_prefix<'a>(
    provider: &InferenceProvider,
    s: &'a str,
) -> &'a str {
    s.strip_prefix(provider.as_ref())
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(s)
}

impl<'de> Deserialize<'de> for ModelId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ModelId::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl Serialize for ModelId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(p) = self.inference_provider() {
            serializer.serialize_str(&format!("{p}/{self}"))
        } else {
            serializer.serialize_str(&self.to_string())
        }
    }
}

impl FromStr for ModelId {
    type Err = MapperError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, '/');
        match (parts.next(), parts.next()) {
            (Some(p_str), Some(m_name)) if !m_name.is_empty() => {
                let p = InferenceProvider::from_str(p_str).map_err(|_| {
                    MapperError::ProviderNotSupported(p_str.to_string())
                })?;
                Self::from_str_and_provider(p, m_name)
            }
            _ => Err(MapperError::InvalidModelName(format!(
                "Model string format error: {s}"
            ))),
        }
    }
}

impl Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelId::ModelIdWithVersion { id, .. } => id.fmt(f),
            ModelId::Bedrock(m) => m.fmt(f),
            ModelId::Ollama(m) => m.fmt(f),
            ModelId::Unknown(m) => m.fmt(f),
        }
    }
}

#[derive(Debug, Clone, Eq)]
pub struct ModelIdWithoutVersion {
    pub(crate) inner: ModelId,
}

impl From<ModelId> for ModelIdWithoutVersion {
    fn from(inner: ModelId) -> Self {
        Self { inner }
    }
}

impl PartialEq for ModelIdWithoutVersion {
    fn eq(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (
                ModelId::ModelIdWithVersion {
                    provider: p1,
                    id: id1,
                },
                ModelId::ModelIdWithVersion {
                    provider: p2,
                    id: id2,
                },
            ) => p1 == p2 && id1.model == id2.model,
            (ModelId::Bedrock(m1), ModelId::Bedrock(m2)) => {
                m1.provider == m2.provider
                    && m1.model == m2.model
                    && m1.bedrock_internal_version
                        == m2.bedrock_internal_version
            }
            (ModelId::Ollama(m1), ModelId::Ollama(m2)) => {
                m1.model == m2.model && m1.tag == m2.tag
            }
            (ModelId::Unknown(m1), ModelId::Unknown(m2)) => m1 == m2,
            _ => false,
        }
    }
}

impl std::hash::Hash for ModelIdWithoutVersion {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match &self.inner {
            ModelId::ModelIdWithVersion { provider, id } => {
                provider.hash(state);
                id.model.hash(state);
            }
            ModelId::Bedrock(m) => {
                m.provider.hash(state);
                m.model.hash(state);
                m.bedrock_internal_version.hash(state);
            }
            ModelId::Ollama(m) => {
                m.model.hash(state);
                m.tag.hash(state);
            }
            ModelId::Unknown(m) => m.hash(state),
        }
    }
}
