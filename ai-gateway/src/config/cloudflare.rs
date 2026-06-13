/// Cloudflare Workers AI credentials from environment.
///
/// Preferred: `CLOUDFLARE_API_KEY_WITH_ACCOUNT_ID=account_id:api_token`
/// Fallback: `CLOUDFLARE_ACCOUNT_ID` + `CLOUDFLARE_API_KEY`
pub fn credentials_from_env() -> Option<(String, String)> {
    if let Ok(combined) = std::env::var("CLOUDFLARE_API_KEY_WITH_ACCOUNT_ID") {
        let (account_id, api_token) = combined.split_once(':')?;
        if !account_id.is_empty() && !api_token.is_empty() {
            return Some((account_id.to_string(), api_token.to_string()));
        }
    }

    let account_id = std::env::var("CLOUDFLARE_ACCOUNT_ID").ok()?;
    let api_token = std::env::var("CLOUDFLARE_API_KEY").ok()?;
    Some((account_id, api_token))
}
