use std::fmt;

use serde::{
    Deserialize,
    de::{self, MapAccess, Visitor},
};

use super::{
    HeliconeConfig, HeliconeFeatures, default_api_key, default_base_url,
    default_websocket_url,
};

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "kebab-case")]
enum Field {
    ApiKey,
    BaseUrl,
    WebsocketUrl,
    Features,
    Authentication,
    Observability,
    #[serde(rename = "__prompts")]
    Prompts,
}

impl<'de> Deserialize<'de> for HeliconeConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct HeliconeConfigVisitor;
        impl<'de> Visitor<'de> for HeliconeConfigVisitor {
            type Value = HeliconeConfig;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct HeliconeConfig")
            }
            fn visit_map<V>(
                self,
                mut map: V,
            ) -> Result<HeliconeConfig, V::Error>
            where
                V: MapAccess<'de>,
            {
                let (
                    mut api_key,
                    mut base_url,
                    mut websocket_url,
                    mut features,
                    mut auth,
                    mut obs,
                    mut prompts,
                ) = (None, None, None, None, None, None, None);
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::ApiKey => {
                            if api_key.is_some() {
                                return Err(de::Error::duplicate_field(
                                    "api_key",
                                ));
                            }
                            api_key = Some(map.next_value()?);
                        }
                        Field::BaseUrl => {
                            if base_url.is_some() {
                                return Err(de::Error::duplicate_field(
                                    "base_url",
                                ));
                            }
                            base_url = Some(map.next_value()?);
                        }
                        Field::WebsocketUrl => {
                            if websocket_url.is_some() {
                                return Err(de::Error::duplicate_field(
                                    "websocket_url",
                                ));
                            }
                            websocket_url = Some(map.next_value()?);
                        }
                        Field::Features => {
                            if features.is_some() {
                                return Err(de::Error::duplicate_field(
                                    "features",
                                ));
                            }
                            features = Some(map.next_value()?);
                        }
                        Field::Authentication => {
                            if auth.is_some() {
                                return Err(de::Error::duplicate_field(
                                    "authentication",
                                ));
                            }
                            auth = Some(map.next_value()?);
                        }
                        Field::Observability => {
                            if obs.is_some() {
                                return Err(de::Error::duplicate_field(
                                    "observability",
                                ));
                            }
                            obs = Some(map.next_value()?);
                        }
                        Field::Prompts => {
                            if prompts.is_some() {
                                return Err(de::Error::duplicate_field(
                                    "prompts",
                                ));
                            }
                            prompts = Some(map.next_value()?);
                        }
                    }
                }
                let features = if let Some(f) = features {
                    f
                } else {
                    match (auth, obs, prompts) {
                        (_, Some(true), Some(true)) => HeliconeFeatures::All,
                        (_, Some(true), Some(false) | None) => {
                            HeliconeFeatures::Observability
                        }
                        (_, Some(false) | None, Some(true)) => {
                            HeliconeFeatures::Prompts
                        }
                        (
                            Some(true),
                            Some(false) | None,
                            Some(false) | None,
                        ) => HeliconeFeatures::Auth,
                        _ => HeliconeFeatures::None,
                    }
                };
                Ok(HeliconeConfig {
                    api_key: api_key.unwrap_or_else(default_api_key),
                    base_url: base_url.unwrap_or_else(default_base_url),
                    websocket_url: websocket_url
                        .unwrap_or_else(default_websocket_url),
                    features,
                })
            }
        }
        deserializer.deserialize_struct(
            "HeliconeConfig",
            &[
                "api_key",
                "base_url",
                "websocket_url",
                "features",
                "authentication",
                "observability",
                "__prompts",
            ],
            HeliconeConfigVisitor,
        )
    }
}
