use chrono::Utc;
use web_browser_login::{
    BrowserLoginTarget, PollOptions, deepseek_ready_url,
    poll_local_storage_value_with_options,
};

use crate::{
    constants::USER_TOKEN_STORAGE_KEY,
    errors::Error,
    session::file::{SessionFile, save_session, session_path_from_env},
};

const TARGET: BrowserLoginTarget = BrowserLoginTarget::new(
    "https://chat.deepseek.com/",
    "https://chat.deepseek.com/",
)
.with_timeout(900);

pub async fn run_login() -> Result<(), Error> {
    let path = session_path_from_env()
        .ok_or(Error::MissingSession(crate::constants::SESSION_ENV))?;
    eprintln!("Log in to DeepSeek in the opened browser (email / OAuth).");
    eprintln!("Up to 15 minutes. Browser stays open — close it yourself.");
    eprintln!(
        "Fallback: DevTools → Application → Local Storage → userToken → \
         `deepseek import --token '...'`"
    );
    let raw = poll_local_storage_value_with_options(
        TARGET,
        USER_TOKEN_STORAGE_KEY,
        deepseek_ready_url,
        PollOptions {
            keep_browser_open: true,
            ..PollOptions::default()
        },
    )
    .await
    .map_err(Error::Other)?;
    save_session_from_token(&path, &raw).await
}

pub async fn save_session_from_token(
    path: &std::path::Path,
    raw_token: &str,
) -> Result<(), Error> {
    save_session(
        path,
        &SessionFile {
            token: raw_token.to_string(),
            saved_at: Utc::now(),
        },
    )
    .await?;
    eprintln!("Session saved to {}", path.display());
    Ok(())
}
