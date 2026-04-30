use super::base::ModelIdWithVersion;
use crate::error::mapper::MapperError;

use super::id::ModelId;
use super::version::Version;
use crate::types::model_id::parsing::parse_date;
use crate::types::provider::InferenceProvider;
use chrono::{DateTime, Datelike, Utc};
use std::str::FromStr;

#[test]
fn groq_model_id_format_with_slash() {
    let groq_model_id_str = "meta-llama/llama-4-maverick-17b-128e-instruct";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Named("groq".into()),
        groq_model_id_str,
    )
    .unwrap();
    let ModelId::ModelIdWithVersion { provider, id } = result else {
        panic!("Expected ModelIdWithVersion with Groq provider");
    };
    assert_eq!(
        id,
        ModelIdWithVersion {
            model: "meta-llama/llama-4-maverick-17b-128e-instruct".to_string(),
            version: Version::ImplicitLatest,
        }
    );
    assert_eq!(provider, InferenceProvider::Named("groq".into()));
}

#[test]
fn groq_model_id_format_without_slash() {
    let groq_model_id_str = "deepseek-r1-distill-llama-70b";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Named("groq".into()),
        groq_model_id_str,
    )
    .unwrap();
    let ModelId::ModelIdWithVersion { provider, id } = result else {
        panic!("Expected ModelIdWithVersion with Groq provider");
    };
    assert_eq!(
        id,
        ModelIdWithVersion {
            model: "deepseek-r1-distill-llama-70b".to_string(),
            version: Version::ImplicitLatest,
        }
    );
    assert_eq!(provider, InferenceProvider::Named("groq".into()));
}

#[test]
fn test_openai_o1_snapshot_model() {
    let model_id_str = "o1-2024-12-17";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::OpenAI, model_id_str)
            .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with OpenAI provider");
    };
    assert!(matches!(provider, InferenceProvider::OpenAI));
    assert_eq!(model_with_version.model, "o1");
    let Version::Date { date, .. } = &model_with_version.version else {
        panic!("Expected date version");
    };
    let expected_dt: DateTime<Utc> = "2024-12-17T00:00:00Z".parse().unwrap();
    assert_eq!(*date, expected_dt);

    assert_eq!(result.to_string(), model_id_str);
}

#[test]
fn test_openai_o1_preview_snapshot_model() {
    let model_id_str = "o1-preview-2024-09-12";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::OpenAI, model_id_str)
            .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with OpenAI provider");
    };
    assert!(matches!(provider, InferenceProvider::OpenAI));
    assert_eq!(model_with_version.model, "o1");
    let Version::DateVersionedPreview { date, .. } =
        &model_with_version.version
    else {
        panic!("Expected date versioned preview");
    };
    let expected_dt: DateTime<Utc> = "2024-09-12T00:00:00Z".parse().unwrap();
    assert_eq!(*date, expected_dt);

    assert_eq!(result.to_string(), model_id_str);
}

#[test]
fn test_openai_gpt4_snapshot_model() {
    let model_id_str = "gpt-4-2024-08-15";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::OpenAI, model_id_str)
            .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with OpenAI provider");
    };
    assert!(matches!(provider, InferenceProvider::OpenAI));
    assert_eq!(model_with_version.model, "gpt-4");
    let Version::Date { date, .. } = &model_with_version.version else {
        panic!("Expected date version");
    };
    let expected_dt: DateTime<Utc> = "2024-08-15T00:00:00Z".parse().unwrap();
    assert_eq!(*date, expected_dt);

    assert_eq!(result.to_string(), model_id_str);
}

#[test]
fn test_openai_gpt35_turbo_snapshot_model() {
    let model_id_str = "gpt-3.5-turbo-2024-01-25";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::OpenAI, model_id_str)
            .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with OpenAI provider");
    };
    assert!(matches!(provider, InferenceProvider::OpenAI));
    assert_eq!(model_with_version.model, "gpt-3.5-turbo");
    let Version::Date { date, .. } = &model_with_version.version else {
        panic!("Expected date version");
    };
    let expected_dt: DateTime<Utc> = "2024-01-25T00:00:00Z".parse().unwrap();
    assert_eq!(*date, expected_dt);

    assert_eq!(result.to_string(), model_id_str);
}

#[test]
fn test_openai_o1_alias_model() {
    let model_id_str = "o1";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::OpenAI, model_id_str)
            .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with OpenAI provider");
    };
    assert!(matches!(provider, InferenceProvider::OpenAI));
    assert_eq!(model_with_version.model, "o1");
    assert!(matches!(
        model_with_version.version,
        Version::ImplicitLatest
    ));

    assert_eq!(result.to_string(), model_id_str);
}

#[test]
fn test_openai_o1_preview_alias_model() {
    let model_id_str = "o1-preview";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::OpenAI, model_id_str)
            .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with OpenAI provider");
    };
    assert!(matches!(provider, InferenceProvider::OpenAI));
    assert_eq!(model_with_version.model, "o1");
    assert!(matches!(model_with_version.version, Version::Preview));

    assert_eq!(result.to_string(), model_id_str);
}

#[test]
fn test_openai_gpt4_alias_model() {
    let model_id_str = "gpt-4";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::OpenAI, model_id_str)
            .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with OpenAI provider");
    };
    assert!(matches!(provider, InferenceProvider::OpenAI));
    assert_eq!(model_with_version.model, "gpt-4");
    assert!(matches!(
        model_with_version.version,
        Version::ImplicitLatest
    ));

    assert_eq!(result.to_string(), model_id_str);
}

#[test]
fn test_openai_gpt35_turbo_alias_model() {
    let model_id_str = "gpt-3.5-turbo";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::OpenAI, model_id_str)
            .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with OpenAI provider");
    };
    assert!(matches!(provider, InferenceProvider::OpenAI));
    assert_eq!(model_with_version.model, "gpt-3.5-turbo");
    assert!(matches!(
        model_with_version.version,
        Version::ImplicitLatest
    ));

    assert_eq!(result.to_string(), model_id_str);
}

#[test]
fn test_anthropic_claude_opus_4_dated_model() {
    let model_id_str = "claude-opus-4-20250514";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Anthropic,
        model_id_str,
    )
    .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with Anthropic provider");
    };
    assert!(matches!(provider, InferenceProvider::Anthropic));
    assert_eq!(model_with_version.model, "claude-opus-4");
    let Version::Date { date, .. } = model_with_version.version else {
        panic!("Expected date version");
    };
    let expected_dt: DateTime<Utc> = "2025-05-14T00:00:00Z".parse().unwrap();
    assert_eq!(date, expected_dt);

    assert_eq!(result.to_string(), model_id_str);
}

#[test]
fn test_anthropic_claude_sonnet_4_dated_model() {
    let model_id_str = "claude-sonnet-4-20250514";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Anthropic,
        model_id_str,
    )
    .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with Anthropic provider");
    };
    assert!(matches!(provider, InferenceProvider::Anthropic));
    assert_eq!(model_with_version.model, "claude-sonnet-4");
    let Version::Date { date, .. } = &model_with_version.version else {
        panic!("Expected date version");
    };
    let expected_dt: DateTime<Utc> = "2025-05-14T00:00:00Z".parse().unwrap();
    assert_eq!(*date, expected_dt);

    assert_eq!(result.to_string(), model_id_str);
}

#[test]
fn test_anthropic_claude_3_7_sonnet_dated_model() {
    let model_id_str = "claude-3-7-sonnet-20250219";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Anthropic,
        model_id_str,
    )
    .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with Anthropic provider");
    };
    assert!(matches!(provider, InferenceProvider::Anthropic));
    assert_eq!(model_with_version.model, "claude-3-7-sonnet");
    let Version::Date { date, .. } = &model_with_version.version else {
        panic!("Expected date version");
    };
    let expected_dt: DateTime<Utc> = "2025-02-19T00:00:00Z".parse().unwrap();
    assert_eq!(*date, expected_dt);

    assert_eq!(result.to_string(), model_id_str);
}

#[test]
fn test_anthropic_claude_3_haiku_dated_model() {
    let model_id_str = "claude-3-haiku-20240307";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Anthropic,
        model_id_str,
    )
    .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with Anthropic provider");
    };
    assert!(matches!(provider, InferenceProvider::Anthropic));
    assert_eq!(model_with_version.model, "claude-3-haiku");
    let Version::Date { date, .. } = &model_with_version.version else {
        panic!("Expected date version");
    };
    let expected_dt: DateTime<Utc> = "2024-03-07T00:00:00Z".parse().unwrap();
    assert_eq!(*date, expected_dt);

    assert_eq!(result.to_string(), model_id_str);
}

#[test]
fn test_anthropic_claude_3_7_sonnet_latest_alias() {
    let model_id_str = "claude-3-7-sonnet-latest";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Anthropic,
        model_id_str,
    )
    .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with Anthropic provider");
    };
    assert!(matches!(provider, InferenceProvider::Anthropic));
    assert_eq!(model_with_version.model, "claude-3-7-sonnet");
    assert!(matches!(model_with_version.version, Version::Latest));

    // Display for Version::Latest appends "-latest"
    assert_eq!(result.to_string(), "claude-3-7-sonnet-latest");
}

#[test]
fn test_anthropic_claude_sonnet_4_latest_alias() {
    let model_id_str = "claude-sonnet-4-latest";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Anthropic,
        model_id_str,
    )
    .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with Anthropic provider");
    };
    assert!(matches!(provider, InferenceProvider::Anthropic));
    assert_eq!(model_with_version.model, "claude-sonnet-4");
    assert!(matches!(model_with_version.version, Version::Latest));

    // Display for Version::Latest appends "-latest"
    assert_eq!(result.to_string(), "claude-sonnet-4-latest");
}

#[test]
fn test_anthropic_claude_opus_4_0_implicit_latest() {
    let model_id_str = "claude-opus-4-0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Anthropic,
        model_id_str,
    )
    .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with Anthropic provider");
    };
    assert!(matches!(provider, InferenceProvider::Anthropic));
    assert_eq!(model_with_version.model, "claude-opus-4-0");
    assert!(matches!(
        model_with_version.version,
        Version::ImplicitLatest
    ));

    assert_eq!(result.to_string(), model_id_str);
}

#[test]
fn test_anthropic_claude_sonnet_4_0_implicit_latest() {
    let model_id_str = "claude-sonnet-4-0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Anthropic,
        model_id_str,
    )
    .unwrap();
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = &result
    else {
        panic!("Expected ModelIdWithVersion with Anthropic provider");
    };
    assert!(matches!(provider, InferenceProvider::Anthropic));
    assert_eq!(model_with_version.model, "claude-sonnet-4-0");
    assert!(matches!(
        model_with_version.version,
        Version::ImplicitLatest
    ));

    assert_eq!(result.to_string(), model_id_str);
}

#[test]
fn test_bedrock_amazon_titan_valid_provider() {
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        "amazon.titan-embed-text-v1:0",
    );
    assert!(result.is_ok());
}

#[test]
fn test_bedrock_ai21_jamba_valid_provider() {
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        "ai21.jamba-1-5-large-v1:0",
    );
    assert!(result.is_ok());
}

#[test]
fn test_bedrock_meta_llama_valid_provider() {
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        "meta.llama3-8b-instruct-v1:0",
    );
    assert!(result.is_ok());
}

#[test]
fn test_bedrock_openai_invalid_format() {
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        "openai.gpt-4:1",
    );
    assert!(result.is_err());
    // This should fail because the format doesn't have `-v` pattern
    // required for Bedrock
    if let Err(MapperError::InvalidModelName(model_name)) = result {
        assert_eq!(model_name, "openai.gpt-4:1");
    } else {
        panic!(
            "Expected InvalidModelName error for OpenAI format on \
             Bedrock, got: {result:?}"
        );
    }
}

#[test]
fn test_bedrock_anthropic_claude_opus_4_model() {
    let model_id_str = "anthropic.claude-opus-4-20250514-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(
            bedrock_model.provider,
            InferenceProvider::Anthropic.to_string()
        );
        assert_eq!(bedrock_model.model, "claude-opus-4");
        let Version::Date { date, .. } = &bedrock_model.version else {
            panic!("Expected date version");
        };
        let expected_dt: DateTime<Utc> =
            "2025-05-14T00:00:00Z".parse().unwrap();
        assert_eq!(*date, expected_dt);
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");

        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Bedrock ModelId with Anthropic provider");
    }
}

#[test]
fn test_bedrock_anthropic_claude_3_7_sonnet_model() {
    let model_id_str = "anthropic.claude-3-7-sonnet-20250219-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(
            bedrock_model.provider,
            InferenceProvider::Anthropic.to_string()
        );
        assert_eq!(bedrock_model.model, "claude-3-7-sonnet");
        let Version::Date { date, .. } = &bedrock_model.version else {
            panic!("Expected date version");
        };
        let expected_dt: DateTime<Utc> =
            "2025-02-19T00:00:00Z".parse().unwrap();
        assert_eq!(*date, expected_dt);
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Bedrock ModelId with Anthropic provider");
    }
}

#[test]
fn test_bedrock_anthropic_claude_3_haiku_model() {
    let model_id_str = "anthropic.claude-3-haiku-20240307-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(
            bedrock_model.provider,
            InferenceProvider::Anthropic.to_string()
        );
        assert_eq!(bedrock_model.model, "claude-3-haiku");
        let Version::Date { date, .. } = &bedrock_model.version else {
            panic!("Expected date version");
        };
        let expected_dt: DateTime<Utc> =
            "2024-03-07T00:00:00Z".parse().unwrap();
        assert_eq!(*date, expected_dt);
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Bedrock ModelId with Anthropic provider");
    }
}

#[test]
fn test_bedrock_anthropic_claude_3_sonnet_valid_provider() {
    let model_id_str = "anthropic.claude-3-sonnet-20240229-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(
            bedrock_model.provider,
            InferenceProvider::Anthropic.to_string()
        );
        assert_eq!(bedrock_model.model, "claude-3-sonnet");
        let Version::Date { date, .. } = &bedrock_model.version else {
            panic!("Expected date version");
        };
        let expected_dt: DateTime<Utc> =
            "2024-02-29T00:00:00Z".parse().unwrap();
        assert_eq!(*date, expected_dt);
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Bedrock ModelId with Anthropic provider");
    }
}

#[test]
fn test_bedrock_anthropic_claude_3_5_sonnet_model() {
    let model_id_str = "anthropic.claude-3-5-sonnet-20241022-v2:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(
            bedrock_model.provider,
            InferenceProvider::Anthropic.to_string()
        );
        assert_eq!(bedrock_model.model, "claude-3-5-sonnet");
        let Version::Date { date, .. } = &bedrock_model.version else {
            panic!("Expected date version");
        };
        let expected_dt: DateTime<Utc> =
            "2024-10-22T00:00:00Z".parse().unwrap();
        assert_eq!(*date, expected_dt);
        assert_eq!(bedrock_model.bedrock_internal_version, "v2:0");
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Bedrock ModelId with Anthropic provider");
    }
}

#[test]
fn test_bedrock_anthropic_claude_sonnet_4_model_proper_format() {
    let model_id_str = "anthropic.claude-sonnet-4-20250514-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(
            bedrock_model.provider,
            InferenceProvider::Anthropic.to_string()
        );
        assert_eq!(bedrock_model.model, "claude-sonnet-4");
        let Version::Date { date, .. } = &bedrock_model.version else {
            panic!("Expected date version");
        };
        let expected_dt: DateTime<Utc> =
            "2025-05-14T00:00:00Z".parse().unwrap();
        assert_eq!(*date, expected_dt);
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Bedrock ModelId with Anthropic provider");
    }
}

#[test]
fn test_ollama_gemma3_basic_model() {
    let model_id_str = "gemma3";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::Ollama, model_id_str);
    assert!(result.is_ok());
    if let Ok(ModelId::Ollama(ollama_model)) = &result {
        assert_eq!(ollama_model.model, "gemma3");
        assert_eq!(ollama_model.tag, None);

        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Ollama ModelId");
    }
}

#[test]
fn test_ollama_llama32_basic_model() {
    let model_id_str = "llama3.2";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::Ollama, model_id_str);
    assert!(result.is_ok());
    if let Ok(ModelId::Ollama(ollama_model)) = &result {
        assert_eq!(ollama_model.model, "llama3.2");
        assert_eq!(ollama_model.tag, None);
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Ollama ModelId");
    }
}

#[test]
fn test_ollama_phi4_mini_basic_model() {
    let model_id_str = "phi4-mini";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::Ollama, model_id_str);
    assert!(result.is_ok());
    if let Ok(ModelId::Ollama(ollama_model)) = &result {
        assert_eq!(ollama_model.model, "phi4-mini");
        assert_eq!(ollama_model.tag, None);
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Ollama ModelId");
    }
}

#[test]
fn test_ollama_llama32_vision_basic_model() {
    let model_id_str = "llama3.2-vision";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::Ollama, model_id_str);
    assert!(result.is_ok());
    if let Ok(ModelId::Ollama(ollama_model)) = &result {
        assert_eq!(ollama_model.model, "llama3.2-vision");
        assert_eq!(ollama_model.tag, None);
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Ollama ModelId");
    }
}

#[test]
fn test_ollama_deepseek_r1_basic_model() {
    let model_id_str = "deepseek-r1";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::Ollama, model_id_str);
    assert!(result.is_ok());
    if let Ok(ModelId::Ollama(ollama_model)) = &result {
        assert_eq!(ollama_model.model, "deepseek-r1");
        assert_eq!(ollama_model.tag, None);
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Ollama ModelId");
    }
}

#[test]
fn test_ollama_gemma3_1b_tagged_model() {
    let model_id_str = "gemma3:1b";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::Ollama, model_id_str);
    assert!(result.is_ok());
    if let Ok(ModelId::Ollama(ollama_model)) = &result {
        assert_eq!(ollama_model.model, "gemma3");
        assert_eq!(ollama_model.tag, Some("1b".to_string()));

        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Ollama ModelId with tag");
    }
}

#[test]
fn test_ollama_gemma3_12b_tagged_model() {
    let model_id_str = "gemma3:12b";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::Ollama, model_id_str);
    assert!(result.is_ok());
    if let Ok(ModelId::Ollama(ollama_model)) = &result {
        assert_eq!(ollama_model.model, "gemma3");
        assert_eq!(ollama_model.tag, Some("12b".to_string()));
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Ollama ModelId with tag");
    }
}

#[test]
fn test_ollama_deepseek_r1_671b_tagged_model() {
    let model_id_str = "deepseek-r1:671b";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::Ollama, model_id_str);
    assert!(result.is_ok());
    if let Ok(ModelId::Ollama(ollama_model)) = &result {
        assert_eq!(ollama_model.model, "deepseek-r1");
        assert_eq!(ollama_model.tag, Some("671b".to_string()));
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Ollama ModelId with tag");
    }
}

#[test]
fn test_ollama_llama4_scout_tagged_model() {
    let model_id_str = "llama4:scout";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::Ollama, model_id_str);
    assert!(result.is_ok());
    if let Ok(ModelId::Ollama(ollama_model)) = &result {
        assert_eq!(ollama_model.model, "llama4");
        assert_eq!(ollama_model.tag, Some("scout".to_string()));
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Ollama ModelId with tag");
    }
}

#[test]
fn test_ollama_llama4_maverick_tagged_model() {
    let model_id_str = "llama4:maverick";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::Ollama, model_id_str);
    assert!(result.is_ok());
    if let Ok(ModelId::Ollama(ollama_model)) = &result {
        assert_eq!(ollama_model.model, "llama4");
        assert_eq!(ollama_model.tag, Some("maverick".to_string()));
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Ollama ModelId with tag");
    }
}

#[test]
fn test_ollama_llama_2_uncensored_freeform() {
    let model_id_str = "Llama 2 Uncensored";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::Ollama, model_id_str);
    assert!(result.is_ok());
    if let Ok(ModelId::Ollama(ollama_model)) = &result {
        assert_eq!(ollama_model.model, "Llama 2 Uncensored");
        assert_eq!(ollama_model.tag, None);
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Ollama ModelId");
    }
}

#[test]
fn test_bedrock_with_geo_field() {
    let model_id_str = "us.anthropic.claude-3-sonnet-20240229-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.geo, Some("us".to_string()));
        assert_eq!(
            bedrock_model.provider,
            InferenceProvider::Anthropic.to_string()
        );
        assert_eq!(bedrock_model.model, "claude-3-sonnet");
        let Version::Date { date, .. } = &bedrock_model.version else {
            panic!("Expected date version");
        };
        let expected_dt: DateTime<Utc> =
            "2024-02-29T00:00:00Z".parse().unwrap();
        assert_eq!(*date, expected_dt);
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Bedrock ModelId with geo field");
    }
}

#[test]
fn test_bedrock_with_geo_field_no_version() {
    let model_id_str = "eu.amazon.titan-embed-text-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.geo, Some("eu".to_string()));
        assert_eq!(bedrock_model.provider, "amazon");
        assert_eq!(bedrock_model.model, "titan-embed-text");
        assert!(matches!(bedrock_model.version, Version::ImplicitLatest));
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Bedrock ModelId with geo field");
    }
}

#[test]
fn test_invalid_bedrock_unknown_provider_model() {
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        "some-unknown-provider.model",
    );

    assert!(result.is_err());
    if let Err(MapperError::InvalidModelName(provider)) = result {
        assert_eq!(provider, "some-unknown-provider.model");
    } else {
        panic!("Expected ProviderNotSupported error for unknown provider");
    }
}

#[test]
fn test_invalid_bedrock_no_dot_separator() {
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        "custom-local-model",
    );
    assert!(result.is_err());
    if let Err(MapperError::InvalidModelName(model_name)) = result {
        assert_eq!(model_name, "custom-local-model");
    } else {
        panic!(
            "Expected InvalidModelName error for model without dot \
             separator"
        );
    }
}

#[test]
fn test_invalid_bedrock_malformed_format() {
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        "experimental@format#unknown",
    );
    assert!(result.is_err());
    if let Err(MapperError::InvalidModelName(model_name)) = result {
        assert_eq!(model_name, "experimental@format#unknown");
    } else {
        panic!("Expected InvalidModelName error for malformed format");
    }
}

#[test]
fn test_edge_case_empty_string() {
    let result = ModelId::from_str_and_provider(InferenceProvider::OpenAI, "");
    assert!(result.is_err());
    if let Err(MapperError::InvalidModelName(msg)) = result {
        assert_eq!(msg, "Model name cannot be empty");
    } else {
        panic!("Expected InvalidModelName error for empty string");
    }
}

#[test]
fn test_edge_case_single_char() {
    let model_id_str = "a";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::OpenAI, model_id_str);
    assert!(result.is_ok());
    if let Ok(ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    }) = &result
    {
        assert!(matches!(provider, InferenceProvider::OpenAI));
        assert_eq!(model_with_version.model, "a");
        assert!(matches!(
            model_with_version.version,
            Version::ImplicitLatest
        ));

        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected OpenAI ModelId for single character");
    }
}

#[test]
fn test_edge_case_trailing_dash() {
    let result =
        ModelId::from_str_and_provider(InferenceProvider::OpenAI, "model-");
    assert!(result.is_err());
    if let Err(MapperError::InvalidModelName(msg)) = result {
        assert_eq!(msg, "Model name cannot end with dash");
    } else {
        panic!("Expected InvalidModelName error for trailing dash");
    }
}

#[test]
fn test_edge_case_at_symbol() {
    let result =
        ModelId::from_str_and_provider(InferenceProvider::OpenAI, "model@");
    assert!(result.is_err());
    if let Err(MapperError::InvalidModelName(msg)) = result {
        assert_eq!(msg, "Model name cannot end with @ symbol");
    } else {
        panic!("Expected InvalidModelName error for @ symbol");
    }
}

#[test]
fn test_edge_case_trailing_dot() {
    let result =
        ModelId::from_str_and_provider(InferenceProvider::OpenAI, "provider.");
    assert!(result.is_err());
    if let Err(MapperError::InvalidModelName(msg)) = result {
        assert_eq!(msg, "Model name cannot end with dot");
    } else {
        panic!("Expected InvalidModelName error for trailing dot");
    }
}

#[test]
fn test_edge_case_at_only() {
    let result = ModelId::from_str_and_provider(InferenceProvider::OpenAI, "@");
    assert!(result.is_err());
    if let Err(MapperError::InvalidModelName(msg)) = result {
        assert_eq!(msg, "Model name cannot end with @ symbol");
    } else {
        panic!("Expected InvalidModelName error for @ only");
    }
}

#[test]
fn test_edge_case_dash_only() {
    let result = ModelId::from_str_and_provider(InferenceProvider::OpenAI, "-");
    assert!(result.is_err());
    if let Err(MapperError::InvalidModelName(msg)) = result {
        assert_eq!(msg, "Model name cannot end with dash");
    } else {
        panic!("Expected InvalidModelName error for dash only");
    }
}

#[test]
fn test_provider_specific_model_variants() {
    let openai_result =
        ModelId::from_str_and_provider(InferenceProvider::OpenAI, "gpt-4");
    assert!(matches!(
        openai_result,
        Ok(ModelId::ModelIdWithVersion {
            provider: InferenceProvider::OpenAI,
            ..
        })
    ));

    let anthropic_result = ModelId::from_str_and_provider(
        InferenceProvider::Anthropic,
        "claude-3-sonnet",
    );
    assert!(matches!(
        anthropic_result,
        Ok(ModelId::ModelIdWithVersion {
            provider: InferenceProvider::Anthropic,
            ..
        })
    ));

    let bedrock_result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        "anthropic.claude-3-sonnet-20240229-v1:0",
    );
    assert!(matches!(bedrock_result, Ok(ModelId::Bedrock(_))));

    let ollama_result =
        ModelId::from_str_and_provider(InferenceProvider::Ollama, "llama3");
    assert!(matches!(ollama_result, Ok(ModelId::Ollama(_))));
}

#[test]
fn test_from_str_openai_model() {
    let model_str = "openai/gpt-4";
    let result = ModelId::from_str(model_str).unwrap();

    if let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = result
    {
        assert!(matches!(provider, InferenceProvider::OpenAI));
        assert_eq!(model_with_version.model, "gpt-4");
        assert!(matches!(
            model_with_version.version,
            Version::ImplicitLatest
        ));
    } else {
        panic!("Expected ModelIdWithVersion with OpenAI provider");
    }
}

#[test]
fn test_from_str_anthropic_model() {
    let model_str = "anthropic/claude-3-sonnet-20240229";
    let result = ModelId::from_str(model_str).unwrap();

    if let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = result
    {
        assert!(matches!(provider, InferenceProvider::Anthropic));
        assert_eq!(model_with_version.model, "claude-3-sonnet");
        if let Version::Date { date, .. } = model_with_version.version {
            let expected_dt: DateTime<Utc> =
                "2024-02-29T00:00:00Z".parse().unwrap();
            assert_eq!(date, expected_dt);
        } else {
            panic!("Expected date version");
        }
    } else {
        panic!("Expected ModelIdWithVersion with Anthropic provider");
    }
}

#[test]
fn test_from_str_anthropic_claude_opus_4_0_model() {
    let model_str = "anthropic/claude-opus-4-0";
    let result = ModelId::from_str(model_str).unwrap();

    if let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = result
    {
        assert!(matches!(provider, InferenceProvider::Anthropic));
        assert_eq!(model_with_version.model, "claude-opus-4-0");
        assert!(matches!(
            model_with_version.version,
            Version::ImplicitLatest
        ));
    } else {
        panic!("Expected ModelIdWithVersion with Anthropic provider");
    }
}

#[test]
fn test_from_str_anthropic_claude_sonnet_4_0_model() {
    let model_str = "anthropic/claude-sonnet-4-0";
    let result = ModelId::from_str(model_str).unwrap();

    if let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = result
    {
        assert!(matches!(provider, InferenceProvider::Anthropic));
        assert_eq!(model_with_version.model, "claude-sonnet-4-0");
        assert!(matches!(
            model_with_version.version,
            Version::ImplicitLatest
        ));
    } else {
        panic!("Expected ModelIdWithVersion with Anthropic provider");
    }
}

#[test]
fn test_from_str_bedrock_model() {
    let model_str = "bedrock/anthropic.claude-3-sonnet-20240229-v1:0";
    let result = ModelId::from_str(model_str).unwrap();

    if let ModelId::Bedrock(bedrock_model) = result {
        assert_eq!(
            bedrock_model.provider,
            InferenceProvider::Anthropic.to_string()
        );
        assert_eq!(bedrock_model.model, "claude-3-sonnet");
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
    } else {
        panic!("Expected Bedrock ModelId");
    }
}

#[test]
fn test_from_str_ollama_model() {
    let model_str = "ollama/llama3:8b";
    let result = ModelId::from_str(model_str).unwrap();

    if let ModelId::Ollama(ollama_model) = result {
        assert_eq!(ollama_model.model, "llama3");
        assert_eq!(ollama_model.tag, Some("8b".to_string()));
    } else {
        panic!("Expected Ollama ModelId");
    }
}

#[test]
fn test_from_str_google_gemini_model() {
    let model_str = "gemini/gemini-pro";
    let result = ModelId::from_str(model_str).unwrap();

    if let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = result
    {
        assert!(matches!(provider, InferenceProvider::GoogleGemini));
        assert_eq!(model_with_version.model, "gemini-pro");
        assert!(matches!(
            model_with_version.version,
            Version::ImplicitLatest
        ));
    } else {
        panic!("Expected ModelIdWithVersion with GoogleGemini provider");
    }
}

#[test]
fn test_from_str_invalid_no_slash() {
    let result = ModelId::from_str("gpt-4");
    assert!(result.is_err());
    if let Err(MapperError::InvalidModelName(msg)) = result {
        assert_eq!(msg, "Model string format error: gpt-4");
    } else {
        panic!("Expected InvalidModelName error");
    }
}

#[test]
fn test_from_str_invalid_empty_model() {
    let result = ModelId::from_str("openai/");
    assert!(result.is_err());
    if let Err(MapperError::InvalidModelName(msg)) = result {
        assert_eq!(msg, "Model string format error: openai/");
    } else {
        panic!("Expected InvalidModelName error");
    }
}

#[test]
fn test_version_implicit_latest_from_empty_string() {
    let version = Version::from_str("").unwrap();
    assert!(matches!(version, Version::ImplicitLatest));
}

#[test]
fn test_version_implicit_latest_display() {
    let version = Version::ImplicitLatest;
    assert_eq!(version.to_string(), "");
}

#[test]
fn test_version_implicit_latest_serialization() {
    let version = Version::ImplicitLatest;
    let serialized = serde_json::to_string(&version).unwrap();
    assert_eq!(serialized, "\"\"");
}

#[test]
fn test_version_implicit_latest_deserialization() {
    let json = "\"\"";
    let version: Version = serde_json::from_str(json).unwrap();
    assert!(matches!(version, Version::ImplicitLatest));
}

#[test]
fn test_version_implicit_latest_roundtrip() {
    let original = Version::ImplicitLatest;
    let serialized = serde_json::to_string(&original).unwrap();
    let deserialized: Version = serde_json::from_str(&serialized).unwrap();
    assert_eq!(original, deserialized);
}

#[test]
fn test_bedrock_mistral_models() {
    // Test Mistral 7B model
    let model_id_str = "mistral.mistral-7b-instruct-v0:2";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.provider, "mistral");
        assert_eq!(bedrock_model.model, "mistral-7b-instruct");
        assert!(matches!(bedrock_model.version, Version::ImplicitLatest));
        assert_eq!(bedrock_model.bedrock_internal_version, "v0:2");
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Bedrock ModelId with Mistral provider");
    }

    // Test Mistral Large model
    let model_id_str = "mistral.mistral-large-2402-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.provider, "mistral");
        assert_eq!(bedrock_model.model, "mistral-large-2402");
        assert!(matches!(bedrock_model.version, Version::ImplicitLatest));
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
    } else {
        panic!("Expected Bedrock ModelId with Mistral provider");
    }
}

#[test]
fn test_bedrock_cohere_models() {
    // Test Cohere Command model
    let model_id_str = "cohere.command-text-v14";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.provider, "cohere");
        assert_eq!(bedrock_model.model, "command-text");
        assert!(matches!(bedrock_model.version, Version::ImplicitLatest));
        assert_eq!(bedrock_model.bedrock_internal_version, "v14");
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Bedrock ModelId with Cohere provider");
    }

    // Test Cohere Command R model
    let model_id_str = "cohere.command-r-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.provider, "cohere");
        assert_eq!(bedrock_model.model, "command-r");
        assert!(matches!(bedrock_model.version, Version::ImplicitLatest));
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Bedrock ModelId with Cohere provider");
    }
}

#[test]
fn test_bedrock_stability_models() {
    // Test Stability AI SDXL model
    let model_id_str = "stability.stable-diffusion-xl-v1";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.provider, "stability");
        assert_eq!(bedrock_model.model, "stable-diffusion-xl");
        assert!(matches!(bedrock_model.version, Version::ImplicitLatest));
        assert_eq!(bedrock_model.bedrock_internal_version, "v1");
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Bedrock ModelId with Stability provider");
    }
}

#[test]
fn test_bedrock_amazon_nova_models() {
    // Test Amazon Nova Pro model
    let model_id_str = "amazon.nova-pro-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.provider, "amazon");
        assert_eq!(bedrock_model.model, "nova-pro");
        assert!(matches!(bedrock_model.version, Version::ImplicitLatest));
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Bedrock ModelId with Amazon provider");
    }

    // Test Amazon Nova Lite model
    let model_id_str = "amazon.nova-lite-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.provider, "amazon");
        assert_eq!(bedrock_model.model, "nova-lite");
        assert!(matches!(bedrock_model.version, Version::ImplicitLatest));
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
    } else {
        panic!("Expected Bedrock ModelId with Amazon provider");
    }
}

#[test]
fn test_bedrock_meta_llama3_models() {
    // Test Llama 3.1 70B model
    let model_id_str = "meta.llama3-1-70b-instruct-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.provider, "meta");
        assert_eq!(bedrock_model.model, "llama3-1-70b-instruct");
        assert!(matches!(bedrock_model.version, Version::ImplicitLatest));
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Bedrock ModelId with Meta provider");
    }

    // Test Llama 3.1 405B model
    let model_id_str = "meta.llama3-1-405b-instruct-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.provider, "meta");
        assert_eq!(bedrock_model.model, "llama3-1-405b-instruct");
        assert!(matches!(bedrock_model.version, Version::ImplicitLatest));
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
    } else {
        panic!("Expected Bedrock ModelId with Meta provider");
    }
}

#[test]
fn test_bedrock_edge_cases() {
    // Test model with multiple dots in the name (will be parsed as
    // geo.provider.model)
    let model_id_str = "provider.model.name.with.dots-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.geo, Some("provider".to_string()));
        assert_eq!(bedrock_model.provider, "model");
        assert_eq!(bedrock_model.model, "name.with.dots");
        assert!(matches!(bedrock_model.version, Version::ImplicitLatest));
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
    } else {
        panic!("Expected Bedrock ModelId");
    }

    // Test model with numbers in version
    let model_id_str = "provider.model-v2:1";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.provider, "provider");
        assert_eq!(bedrock_model.model, "model");
        assert!(matches!(bedrock_model.version, Version::ImplicitLatest));
        assert_eq!(bedrock_model.bedrock_internal_version, "v2:1");
    } else {
        panic!("Expected Bedrock ModelId");
    }

    // Test model with hyphenated provider name
    let model_id_str = "provider-name.model-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.provider, "provider-name");
        assert_eq!(bedrock_model.model, "model");
        assert!(matches!(bedrock_model.version, Version::ImplicitLatest));
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
    } else {
        panic!("Expected Bedrock ModelId");
    }
}

#[test]
fn test_bedrock_invalid_cases() {
    // Test missing version suffix
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        "provider.model",
    );
    assert!(result.is_err());
    if let Err(MapperError::InvalidModelName(model_name)) = result {
        assert_eq!(model_name, "provider.model");
    } else {
        panic!("Expected InvalidModelName error for missing version");
    }

    // Test model with version but no colon (actually valid for some
    // providers)
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        "provider.model-v1",
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.provider, "provider");
        assert_eq!(bedrock_model.model, "model");
        assert!(matches!(bedrock_model.version, Version::ImplicitLatest));
        assert_eq!(bedrock_model.bedrock_internal_version, "v1");
    } else {
        panic!("Expected Bedrock ModelId");
    }

    // Test empty provider (will actually parse with empty string provider)
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        ".model-v1:0",
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.provider, "");
        assert_eq!(bedrock_model.model, "model");
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
    } else {
        panic!("Expected Bedrock ModelId with empty provider");
    }

    // Test model starting with dash (will parse dash as part of model name)
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        "provider.-model-v1:0",
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.provider, "provider");
        assert_eq!(bedrock_model.model, "-model");
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
    } else {
        panic!("Expected Bedrock ModelId");
    }
}

#[test]
fn test_bedrock_geo_with_various_providers() {
    // Test geo with Mistral
    let model_id_str = "eu-west-1.mistral.mistral-large-2402-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.geo, Some("eu-west-1".to_string()));
        assert_eq!(bedrock_model.provider, "mistral");
        assert_eq!(bedrock_model.model, "mistral-large-2402");
        assert!(matches!(bedrock_model.version, Version::ImplicitLatest));
        assert_eq!(bedrock_model.bedrock_internal_version, "v1:0");
        assert_eq!(result.as_ref().unwrap().to_string(), model_id_str);
    } else {
        panic!("Expected Bedrock ModelId with geo field");
    }

    // Test geo with Cohere
    let model_id_str = "ap-southeast-1.cohere.command-r-v1:0";
    let result = ModelId::from_str_and_provider(
        InferenceProvider::Bedrock,
        model_id_str,
    );
    assert!(result.is_ok());
    if let Ok(ModelId::Bedrock(bedrock_model)) = &result {
        assert_eq!(bedrock_model.geo, Some("ap-southeast-1".to_string()));
        assert_eq!(bedrock_model.provider, "cohere");
        assert_eq!(bedrock_model.model, "command-r");
    } else {
        panic!("Expected Bedrock ModelId with geo field");
    }
}

#[test]
fn test_parse_date_mmdd_format() {
    // Test MMDD format parsing
    let current_year = chrono::Utc::now().year();

    // Test a valid MMDD date
    let test_date = "0315"; // March 15
    if let Some((parsed_date, format)) = parse_date(test_date) {
        assert_eq!(format, "%m%d");
        assert_eq!(parsed_date.year(), current_year);
        assert_eq!(parsed_date.month(), 3);
        assert_eq!(parsed_date.day(), 15);
    } else {
        panic!("Failed to parse MMDD date");
    }

    // Test another MMDD date
    let test_date = "1225"; // December 25
    if let Some((parsed_date, format)) = parse_date(test_date) {
        assert_eq!(format, "%m%d");
        assert_eq!(parsed_date.year(), current_year);
        assert_eq!(parsed_date.month(), 12);
        assert_eq!(parsed_date.day(), 25);
    } else {
        panic!("Failed to parse MMDD date");
    }
}

#[test]
fn test_model_with_mmdd_date_version() {
    // Test a model with MMDD date version
    let model_id_str = "gpt-4-0125";
    let result =
        ModelId::from_str_and_provider(InferenceProvider::OpenAI, model_id_str);

    assert!(result.is_ok());
    let ModelId::ModelIdWithVersion {
        provider,
        id: model_with_version,
    } = result.unwrap()
    else {
        panic!("Expected ModelIdWithVersion");
    };

    assert!(matches!(provider, InferenceProvider::OpenAI));
    assert_eq!(model_with_version.model, "gpt-4");

    let Version::Date { date, format } = &model_with_version.version else {
        panic!("Expected date version");
    };

    assert_eq!(format, &"%m%d");
    assert_eq!(date.year(), chrono::Utc::now().year());
    assert_eq!(date.month(), 1);
    assert_eq!(date.day(), 25);

    assert_eq!(model_with_version.to_string(), model_id_str);
}
