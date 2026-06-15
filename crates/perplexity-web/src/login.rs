//! Headed browser login — same flow as `chatgpt-web` / OmniRoute DevTools
//! import.

use chrono::Utc;
use web_browser_login::{
    BrowserLoginTarget, PollOptions, perplexity_domain,
    poll_session_cookie_with_options,
};

use crate::{
    Error,
    session::{
        cookie::{build_session_cookie_header, format_login_cookie_pairs},
        file::{SessionFile, save_session, session_path_from_env},
    },
};

const TARGET: BrowserLoginTarget = BrowserLoginTarget::new(
    "https://www.perplexity.ai/",
    "https://www.perplexity.ai/",
)
.with_timeout(900);

pub async fn run_login() -> Result<(), Error> {
    let path = session_path_from_env().ok_or(Error::MissingSession)?;
    eprintln!(
        "Log in to Perplexity in the opened browser (email → 2FA → password)."
    );
    eprintln!("Up to 15 minutes. Browser stays open — close it yourself.");
    eprintln!(
        "Fallback: DevTools → Cookie header → `perplexity import --cookie \
         '...'`"
    );
    let cookie = poll_session_cookie_with_options(
        TARGET,
        perplexity_domain,
        format_login_cookie_pairs,
        |_| false,
        PollOptions {
            keep_browser_open: true,
            ..PollOptions::default()
        },
    )
    .await
    .map_err(Error::Other)?;
    save_session_from_cookie(&path, &cookie).await
}

pub async fn save_session_from_cookie(
    path: &std::path::Path,
    raw_cookie: &str,
) -> Result<(), Error> {
    save_session(
        path,
        &SessionFile {
            cookie: build_session_cookie_header(raw_cookie),
            saved_at: Utc::now(),
        },
    )
    .await?;
    eprintln!("Session saved to {}", path.display());
    Ok(())
}
