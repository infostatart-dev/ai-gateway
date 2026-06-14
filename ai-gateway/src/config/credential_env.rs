use crate::types::{
    provider::{InferenceProvider, ProviderKey},
    secret::Secret,
};

/// Universal secret env var: `AI_GATEWAY_CREDENTIAL_<ID>`.
/// Example: credential id `gemini-free` → `AI_GATEWAY_CREDENTIAL_GEMINI_FREE`.
#[must_use]
pub fn credential_env_var_name(credential_id: &str) -> String {
    format!(
        "AI_GATEWAY_CREDENTIAL_{}",
        credential_id.replace('-', "_").to_ascii_uppercase()
    )
}

#[must_use]
pub fn legacy_provider_env_var_name(provider: &InferenceProvider) -> String {
    format!("{}_API_KEY", provider.to_string().to_ascii_uppercase())
}

pub fn resolve_credential_secret(
    credential_id: &str,
    provider: &InferenceProvider,
    extra_env_names: &[String],
) -> Option<ProviderKey> {
    let names = credential_env_names(credential_id, provider, extra_env_names);

    if *provider == InferenceProvider::Named("cloudflare".into()) {
        return read_cloudflare_key(names);
    }

    read_secret_from_env(names)
}

fn credential_env_names(
    credential_id: &str,
    provider: &InferenceProvider,
    extra_env_names: &[String],
) -> Vec<String> {
    let mut names = vec![credential_env_var_name(credential_id)];
    names.extend(extra_env_names.iter().cloned());

    if credential_id.ends_with("-default") {
        names.push(legacy_provider_env_var_name(provider));
    }

    if *provider == InferenceProvider::Named("cloudflare".into()) {
        names.push("CLOUDFLARE_API_KEY_WITH_ACCOUNT_ID".into());
    } else if *provider == InferenceProvider::Named("gemini".into())
        || *provider == InferenceProvider::GoogleGemini
    {
        match credential_id {
            "gemini-free" => {
                names.push("GEMINI_FREE_TIER_API_KEY".into());
                names.push("GEMINI_FREE_TIER_APIKEY".into());
            }
            "gemini-default" => names.push("GEMINI_API_KEY".into()),
            _ => {}
        }
    }

    names
}

fn read_secret_from_env(names: impl IntoIterator<Item = String>) -> Option<ProviderKey> {
    for name in names {
        if let Ok(value) = std::env::var(&name) {
            if !value.is_empty() {
                return Some(ProviderKey::Secret(Secret::from(value)));
            }
        }
    }
    None
}

fn read_cloudflare_key(names: Vec<String>) -> Option<ProviderKey> {
    for name in names {
        if let Ok(value) = std::env::var(&name) {
            if let Some(key) = cloudflare_token_from_combined(&value) {
                return Some(key);
            }
        }
    }

    crate::config::cloudflare::credentials_from_env()
        .map(|(_, api_token)| ProviderKey::Secret(Secret::from(api_token)))
}

fn cloudflare_token_from_combined(value: &str) -> Option<ProviderKey> {
    let (_, api_token) = value.split_once(':')?;
    if api_token.is_empty() {
        return None;
    }
    Some(ProviderKey::Secret(Secret::from(api_token.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_env_var_name_normalizes_id() {
        assert_eq!(
            credential_env_var_name("gemini-free"),
            "AI_GATEWAY_CREDENTIAL_GEMINI_FREE"
        );
        assert_eq!(
            credential_env_var_name("openrouter-default"),
            "AI_GATEWAY_CREDENTIAL_OPENROUTER_DEFAULT"
        );
    }

    #[test]
    #[serial_test::serial(env)]
    fn universal_env_takes_precedence_over_legacy() {
        unsafe {
            std::env::set_var("AI_GATEWAY_CREDENTIAL_GEMINI_FREE", "universal-key");
            std::env::set_var("GEMINI_FREE_TIER_APIKEY", "legacy-key");
        }
        let key = resolve_credential_secret(
            "gemini-free",
            &InferenceProvider::GoogleGemini,
            &[],
        )
        .unwrap();
        assert_eq!(key.as_secret().unwrap().expose(), "universal-key");
        unsafe {
            std::env::remove_var("AI_GATEWAY_CREDENTIAL_GEMINI_FREE");
            std::env::remove_var("GEMINI_FREE_TIER_APIKEY");
        }
    }

    #[test]
    #[serial_test::serial(env)]
    fn legacy_provider_api_key_still_works_for_default_slot() {
        unsafe {
            std::env::remove_var("AI_GATEWAY_CREDENTIAL_GROQ_DEFAULT");
            std::env::set_var("GROQ_API_KEY", "legacy-groq");
        }
        let key = resolve_credential_secret(
            "groq-default",
            &InferenceProvider::Named("groq".into()),
            &[],
        )
        .unwrap();
        assert_eq!(key.as_secret().unwrap().expose(), "legacy-groq");
        unsafe {
            std::env::remove_var("GROQ_API_KEY");
        }
    }

    #[test]
    #[serial_test::serial(env)]
    fn cloudflare_universal_env_uses_account_token_format() {
        unsafe {
            std::env::set_var(
                "AI_GATEWAY_CREDENTIAL_CLOUDFLARE_DEFAULT",
                "acct123:cfut_secret",
            );
            std::env::remove_var("CLOUDFLARE_API_KEY_WITH_ACCOUNT_ID");
        }
        let key = resolve_credential_secret(
            "cloudflare-default",
            &InferenceProvider::Named("cloudflare".into()),
            &[],
        )
        .unwrap();
        assert_eq!(key.as_secret().unwrap().expose(), "cfut_secret");
        unsafe {
            std::env::remove_var("AI_GATEWAY_CREDENTIAL_CLOUDFLARE_DEFAULT");
        }
    }
}
