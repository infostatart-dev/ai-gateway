use std::collections::HashMap;
use std::time::Duration;

use chromiumoxide::browser::Browser;
use futures::StreamExt;

use crate::{
    browser::browser_config, config::BrowserLoginTarget, options::PollOptions,
};

const MIN_LOGIN_MS: u64 = 5_000;
const POLL_MS: u64 = 1_000;
const STATUS_EVERY_MS: u64 = 20_000;

fn merge_cookie_pairs(
    browser_pairs: &[(String, String)],
    page_pairs: &[(String, String)],
) -> Vec<(String, String)> {
    let mut out: HashMap<String, String> = HashMap::new();
    for (name, value) in browser_pairs.iter().chain(page_pairs.iter()) {
        out.insert(name.clone(), value.clone());
    }
    out.into_iter().collect()
}

/// Poll headed Chromium until `format_pairs` returns a session cookie string.
pub async fn poll_session_cookie(
    target: BrowserLoginTarget,
    domain_ok: impl Fn(&str) -> bool,
    format_pairs: impl Fn(&[(String, String)]) -> Option<String>,
    should_navigate_home: impl Fn(&str) -> bool,
) -> Result<String, String> {
    poll_session_cookie_with_options(
        target,
        domain_ok,
        format_pairs,
        should_navigate_home,
        PollOptions::default(),
    )
    .await
}

/// Like [`poll_session_cookie`], but optionally leaves the browser open after capture.
pub async fn poll_session_cookie_with_options(
    target: BrowserLoginTarget,
    domain_ok: impl Fn(&str) -> bool,
    format_pairs: impl Fn(&[(String, String)]) -> Option<String>,
    should_navigate_home: impl Fn(&str) -> bool,
    options: PollOptions,
) -> Result<String, String> {
    let config =
        browser_config(options.user_data_dir, options.chrome_executable)?;

    let (mut browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| format!("failed to launch browser: {e}"))?;

    let handle =
        tokio::spawn(async move { while handler.next().await.is_some() {} });

    let page = browser
        .new_page(target.login_url)
        .await
        .map_err(|e| format!("failed to open {}: {e}", target.login_url))?;

    let deadline =
        tokio::time::Instant::now() + Duration::from_secs(target.timeout_secs);
    let min_deadline =
        tokio::time::Instant::now() + Duration::from_millis(MIN_LOGIN_MS);
    let mut did_navigate_home = false;
    let mut last_status = tokio::time::Instant::now();

    let cookie = async {
        loop {
            if tokio::time::Instant::now() > deadline {
                return Err(
                    "login timed out — copy Cookie from DevTools and use import"
                        .into(),
                );
            }

            if tokio::time::Instant::now() >= min_deadline {
                let url = match page.url().await {
                    Ok(u) => u.unwrap_or_default(),
                    Err(e) if e.to_string().contains("receiver is gone") => {
                        tokio::time::sleep(Duration::from_millis(POLL_MS))
                            .await;
                        continue;
                    }
                    Err(e) => return Err(e.to_string()),
                };
                if !did_navigate_home && should_navigate_home(&url) {
                    let _ = page.goto(target.home_url).await;
                    did_navigate_home = true;
                }

                let browser_cookies = match browser.get_cookies().await {
                    Ok(c) => c,
                    Err(e) if e.to_string().contains("receiver is gone") => {
                        tokio::time::sleep(Duration::from_millis(POLL_MS))
                            .await;
                        continue;
                    }
                    Err(e) => return Err(e.to_string()),
                };
                let page_cookies = page.get_cookies().await.unwrap_or_default();
                let browser_pairs: Vec<(String, String)> = browser_cookies
                    .into_iter()
                    .filter(|c| domain_ok(&c.domain))
                    .map(|c| (c.name, c.value))
                    .collect();
                let page_pairs: Vec<(String, String)> = page_cookies
                    .into_iter()
                    .filter(|c| domain_ok(&c.domain))
                    .map(|c| (c.name, c.value))
                    .collect();
                let pairs = merge_cookie_pairs(&browser_pairs, &page_pairs);

                if last_status.elapsed()
                    >= Duration::from_millis(STATUS_EVERY_MS)
                {
                    if let Some(fmt) = options.status_line {
                        eprintln!("Waiting for login… {}", fmt(&pairs));
                    }
                    last_status = tokio::time::Instant::now();
                }

                if let Some(cookie) = format_pairs(&pairs) {
                    return Ok(cookie);
                }
            }

            tokio::time::sleep(Duration::from_millis(POLL_MS)).await;
        }
    }
    .await;

    match cookie {
        Ok(cookie) if options.keep_browser_open => {
            std::mem::forget(browser);
            eprintln!(
                "Session captured — close the browser when you are done."
            );
            Ok(cookie)
        }
        Ok(cookie) => {
            browser.close().await.ok();
            handle.abort();
            Ok(cookie)
        }
        Err(err) => {
            browser.close().await.ok();
            handle.abort();
            Err(err)
        }
    }
}
