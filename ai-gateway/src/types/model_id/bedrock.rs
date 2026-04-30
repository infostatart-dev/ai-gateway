use super::parsing::parse_model_and_version;
use super::version::Version;
use crate::error::mapper::MapperError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt::{self, Display},
    str::FromStr,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BedrockModelId {
    pub geo: Option<String>,
    pub provider: String,
    pub model: String,
    pub version: Version,
    pub bedrock_internal_version: String,
}

impl FromStr for BedrockModelId {
    type Err = MapperError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let dot_count = s.chars().filter(|&c| c == '.').count();
        let (geo, provider, rest) = if dot_count >= 2 {
            let mut parts = s.splitn(3, '.');
            (
                Some(parts.next().unwrap().to_string()),
                parts.next().unwrap(),
                parts.next().unwrap(),
            )
        } else if dot_count == 1 {
            let mut parts = s.splitn(2, '.');
            (None, parts.next().unwrap(), parts.next().unwrap())
        } else {
            return Err(MapperError::InvalidModelName(s.to_string()));
        };

        let (model_part, bedrock_version) =
            if let Some(v_pos) = rest.rfind("-v") {
                (&rest[..v_pos], &rest[v_pos + 1..])
            } else {
                return Err(MapperError::InvalidModelName(s.to_string()));
            };

        let (model, version) = parse_model_and_version(model_part, '-');
        Ok(BedrockModelId {
            geo,
            provider: provider.to_string(),
            model: model.to_string(),
            version: version.unwrap_or(Version::ImplicitLatest),
            bedrock_internal_version: bedrock_version.to_string(),
        })
    }
}

impl Display for BedrockModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let geo_str = self
            .geo
            .as_ref()
            .map(|g| format!("{g}."))
            .unwrap_or_default();
        let version_str = match &self.version {
            Version::ImplicitLatest => String::new(),
            v => format!("{v}-"),
        };
        write!(
            f,
            "{}{}.{}{}-{}",
            geo_str,
            self.provider,
            self.model,
            version_str,
            self.bedrock_internal_version
        )
    }
}

impl<'de> Deserialize<'de> for BedrockModelId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        BedrockModelId::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl Serialize for BedrockModelId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
