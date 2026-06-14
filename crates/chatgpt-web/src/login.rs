//! Headed browser login — extracts ChatGPT session cookies into `CHATGPT_BROWSER_CLI` path.
//!
//! OmniRoute uses Electron/Playwright with **`context.cookies()`** (full jar), not page-only
//! cookies. Manual DevTools paste of the full Cookie header is also supported — see `import`.

use std::time::Duration;

use chrono::Utc;
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;

use crate::session::cookie::{build_session_cookie_header, format_login_cookie_pairs};
use crate::session::file::{save_session, session_path_from_env, SessionFile};
use crate::Error;

const LOGIN_URL: &str = "https://chatgpt.com/auth/login";
const HOME_URL: &str = "https://chatgpt.com/";
const MIN_LOGIN_MS: u64 = 5_000;
const POLL_MS: u64 = 1_000;

pub async fn run_login() -> Result<(), Error> {
    let path = session_path_from_env().ok_or(Error::MissingSession)?;
    let cookie = browser_login().await?;
    save_session_from_cookie(&path, &cookie).await
}

pub async fn save_session_from_cookie(path: &std::path::Path, raw_cookie: &str) -> Result<(), Error> {
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

async fn browser_login() -> Result<String, Error> {
    let config = BrowserConfig::builder()
        .with_head()
        .hide()
        .viewport(None)
        .window_size(1280, 800)
        .build()
        .map_err(|e| Error::Other(e.to_string()))?;
    let (mut browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| Error::Other(format!("failed to launch browser: {e}")))?;

    let handle = tokio::spawn(async move {
        while let Some(h) = handler.next().await {
            if h.is_err() {
                break;
            }
        }
    });

    let page = browser
        .new_page(LOGIN_URL)
        .await
        .map_err(|e| Error::Other(format!("failed to open login page: {e}")))?;

    eprintln!("Log in to ChatGPT in the opened browser (email + password / Google).");
    eprintln!("After login you should land on chatgpt.com — waiting up to 5 minutes…");
    eprintln!("Tip: if this hangs, use `chatgpt import` with cookies from Firefox DevTools.");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
    let min_deadline = tokio::time::Instant::now() + Duration::from_millis(MIN_LOGIN_MS);
    loop {
        if tokio::time::Instant::now() > deadline {
            handle.abort();
            return Err(Error::Other(
                "login timed out — copy cookies manually and run: chatgpt import".into(),
            ));
        }

        if tokio::time::Instant::now() >= min_deadline {
            let url = page
                .url()
                .await
                .map_err(|e| Error::Other(e.to_string()))?
                .unwrap_or_default();
            if !url.contains("/auth/login") && url.contains("chatgpt.com") {
                let _ = page.goto(HOME_URL).await;
            }

            let cookies = browser
                .get_cookies()
                .await
                .map_err(|e| Error::Other(e.to_string()))?;
            let pairs: Vec<(String, String)> = cookies
                .into_iter()
                .filter(|c| c.domain.contains("chatgpt.com") || c.domain.contains("openai.com"))
                .map(|c| (c.name, c.value))
                .collect();

            if let Some(cookie) = format_login_cookie_pairs(&pairs) {
                browser.close().await.ok();
                handle.abort();
                return Ok(cookie);
            }
        }

        tokio::time::sleep(Duration::from_millis(POLL_MS)).await;
    }
}
