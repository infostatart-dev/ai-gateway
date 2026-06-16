//! Headed browser login — extracts ChatGPT session cookies into
//! `CHATGPT_BROWSER_CLI` path.
//!
//! OmniRoute uses Electron/Playwright with **`context.cookies()`** (full jar),
//! not page-only cookies. Manual DevTools paste of the full Cookie header is
//! also supported — see `import`.

use chrono::Utc;
use web_browser_login::{
    BrowserLoginTarget, chatgpt_domain, chatgpt_left_login, poll_session_cookie,
};

use crate::{
    Error,
    session::{
        cookie::{build_session_cookie_header, format_login_cookie_pairs},
        file::{SessionFile, save_session, session_path_from_env},
    },
};

const TARGET: BrowserLoginTarget = BrowserLoginTarget::new(
    "https://chatgpt.com/auth/login",
    "https://chatgpt.com/",
);

pub async fn run_login() -> Result<(), Error> {
    let path = session_path_from_env().ok_or(Error::MissingSession)?;
    run_login_to(&path).await
}

pub async fn run_login_to(path: &std::path::Path) -> Result<(), Error> {
    eprintln!(
        "Log in to ChatGPT in the opened browser (email + password / Google)."
    );
    eprintln!(
        "Fallback: Firefox DevTools → Cookie header → `chatgpt import \
         --cookie '...'`"
    );
    let cookie = poll_session_cookie(
        TARGET,
        chatgpt_domain,
        format_login_cookie_pairs,
        chatgpt_left_login,
    )
    .await
    .map_err(Error::Other)?;
    save_session_from_cookie(path, &cookie).await
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
