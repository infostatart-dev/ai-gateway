/// Cloudflare Workers AI credentials: `account_id:api_token` in one value.
#[must_use]
pub fn parse_combined(combined: &str) -> Option<(String, String)> {
    let (account_id, api_token) = combined.split_once(':')?;
    if account_id.is_empty() || api_token.is_empty() {
        return None;
    }
    Some((account_id.to_string(), api_token.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_combined_splits_account_and_token() {
        let (account, token) = parse_combined("acct123:cfut_secret").unwrap();
        assert_eq!(account, "acct123");
        assert_eq!(token, "cfut_secret");
    }

    #[test]
    fn parse_combined_rejects_empty_parts() {
        assert!(parse_combined(":token").is_none());
        assert!(parse_combined("account:").is_none());
    }
}
