use std::{
    fmt::{self, Display},
    str::FromStr,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::mapper::MapperError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OllamaModelId {
    pub model: String,
    pub tag: Option<String>,
}

impl FromStr for OllamaModelId {
    type Err = MapperError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, ':');
        let model = parts
            .next()
            .ok_or_else(|| MapperError::InvalidModelName(s.to_string()))?;
        let tag = parts
            .next()
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string());
        Ok(OllamaModelId {
            model: model.to_string(),
            tag,
        })
    }
}

impl Display for OllamaModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.tag {
            Some(tag) => write!(f, "{}:{}", self.model, tag),
            None => write!(f, "{}", self.model),
        }
    }
}

impl<'de> Deserialize<'de> for OllamaModelId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        OllamaModelId::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl Serialize for OllamaModelId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
