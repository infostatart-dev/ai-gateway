/// Preferred: `AI_GATEWAY_CREDENTIAL_CLOUDFLARE_DEFAULT=account_id:api_token`
/// Legacy: `CLOUDFLARE_API_KEY_WITH_ACCOUNT_ID=account_id:api_token`
/// Fallback: `CLOUDFLARE_ACCOUNT_ID` + `CLOUDFLARE_API_KEY`
#[must_use]
pub fn credentials_from_env() -> Option<(String, String)> {
    for name in [
        "AI_GATEWAY_CREDENTIAL_CLOUDFLARE_DEFAULT",
        "CLOUDFLARE_API_KEY_WITH_ACCOUNT_ID",
    ] {
        if let Ok(combined) = std::env::var(name)
            && let Some(parsed) = parse_combined(&combined)
        {
            return Some(parsed);
        }
    }

    let account_id = std::env::var("CLOUDFLARE_ACCOUNT_ID").ok()?;
    let api_token = std::env::var("CLOUDFLARE_API_KEY").ok()?;
    Some((account_id, api_token))
}

fn parse_combined(combined: &str) -> Option<(String, String)> {
    let (account_id, api_token) = combined.split_once(':')?;
    if account_id.is_empty() || api_token.is_empty() {
        return None;
    }
    Some((account_id.to_string(), api_token.to_string()))
}
