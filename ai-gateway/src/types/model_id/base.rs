use super::parsing::parse_model_and_version;
use super::version::Version;
use crate::error::mapper::MapperError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt::{self, Display},
    str::FromStr,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelIdWithVersion {
    pub model: String,
    pub version: Version,
}

impl FromStr for ModelIdWithVersion {
    type Err = MapperError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(MapperError::InvalidModelName(
                "Model name cannot be empty".to_string(),
            ));
        }
        if s.ends_with('-') || s.ends_with('.') || s.ends_with('@') {
            return Err(MapperError::InvalidModelName(format!(
                "Model name cannot end with {}",
                s.chars().last().unwrap()
            )));
        }
        let (model, version) = parse_model_and_version(s, '-');
        Ok(ModelIdWithVersion {
            model: model.to_string(),
            version: version.unwrap_or(Version::ImplicitLatest),
        })
    }
}

impl Display for ModelIdWithVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.version {
            Version::ImplicitLatest => write!(f, "{}", self.model),
            _ => write!(f, "{}-{}", self.model, self.version),
        }
    }
}

impl<'de> Deserialize<'de> for ModelIdWithVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ModelIdWithVersion::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl Serialize for ModelIdWithVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
