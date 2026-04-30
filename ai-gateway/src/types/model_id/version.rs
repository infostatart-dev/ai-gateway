use std::{fmt::{self, Display}, str::FromStr};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use crate::error::mapper::MapperError;
use super::parsing::parse_date;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Version {
    ImplicitLatest,
    Latest,
    Preview,
    DateVersionedPreview { date: DateTime<Utc>, format: &'static str },
    Date { date: DateTime<Utc>, format: &'static str },
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        Version::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_str(&self.to_string())
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Version::ImplicitLatest => write!(f, ""),
            Version::Latest => write!(f, "latest"),
            Version::Preview => write!(f, "preview"),
            Version::DateVersionedPreview { date, format } => write!(f, "preview-{}", date.format(format)),
            Version::Date { date, format } => write!(f, "{}", date.format(format)),
        }
    }
}

impl FromStr for Version {
    type Err = MapperError;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.eq_ignore_ascii_case("latest") {
            Ok(Version::Latest)
        } else if input.eq_ignore_ascii_case("preview") {
            Ok(Version::Preview)
        } else if let Some(rest) = input.strip_prefix("preview-") {
            if let Some((dt, fmt)) = parse_date(rest) {
                Ok(Version::DateVersionedPreview { date: dt, format: fmt })
            } else {
                Err(MapperError::InvalidModelName(input.to_string()))
            }
        } else if let Some((dt, fmt)) = parse_date(input) {
            Ok(Version::Date { date: dt, format: fmt })
        } else if input.is_empty() {
            Ok(Version::ImplicitLatest)
        } else {
            Err(MapperError::InvalidModelName(input.to_string()))
        }
    }
}
