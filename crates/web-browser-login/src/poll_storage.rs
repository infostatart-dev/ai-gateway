use std::time::Duration;

use chromiumoxide::browser::Browser;
use futures::StreamExt;

use crate::{
    browser::browser_config, config::BrowserLoginTarget, options::PollOptions,
};

const MIN_LOGIN_MS: u64 = 5_000;
const POLL_MS: u64 = 1_000;
const STATUS_EVERY_MS: u64 = 20_000;

/// Normalize a raw `localStorage.getItem` JSON value into an optional string.
#[must_use]
pub(crate) fn normalize_storage_value(
    value: serde_json::Value,
) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) if s.trim().is_empty() => None,
        serde_json::Value::String(s) => Some(s),
        other => Some(other.to_string()),
    }
}

async fn read_local_storage(
    page: &chromiumoxide::Page,
    key: &str,
) -> Result<Option<String>, String> {
    let expr = format!("() => localStorage.getItem({key:?})");
    let result = page
        .evaluate_function(expr)
        .await
        .map_err(|e| e.to_string())?;
    let value: serde_json::Value = match result.into_value() {
        Ok(v) => v,
        Err(e) if e.to_string().contains("No value found") => {
            serde_json::Value::Null
        }
        Err(e) => return Err(e.to_string()),
    };
    match value {
        serde_json::Value::Null => Ok(None),
        other => Ok(normalize_storage_value(other)),
    }
}

/// Poll headed Chromium until `localStorage` contains a non-empty value for
/// `key`.
pub async fn poll_local_storage_value_with_options(
    target: BrowserLoginTarget,
    storage_key: &str,
    should_capture: impl Fn(&str) -> bool,
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
    let mut last_status = tokio::time::Instant::now();

    let token = async {
        loop {
            if tokio::time::Instant::now() > deadline {
                return Err(format!(
                    "login timed out — copy localStorage.{storage_key} from \
                     DevTools and use import"
                ));
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

                if !should_capture(&url) {
                    tokio::time::sleep(Duration::from_millis(POLL_MS)).await;
                    continue;
                }

                if last_status.elapsed()
                    >= Duration::from_millis(STATUS_EVERY_MS)
                {
                    eprintln!("Waiting for login… ({url})");
                    last_status = tokio::time::Instant::now();
                }

                match read_local_storage(&page, storage_key).await {
                    Ok(Some(value)) => return Ok(value),
                    Ok(None) => {}
                    Err(e) if e.contains("receiver is gone") => {}
                    Err(e) => return Err(e),
                }
            }

            tokio::time::sleep(Duration::from_millis(POLL_MS)).await;
        }
    }
    .await;

    match token {
        Ok(value) if options.keep_browser_open => {
            std::mem::forget(browser);
            eprintln!("Token captured — close the browser when you are done.");
            Ok(value)
        }
        Ok(value) => {
            browser.close().await.ok();
            handle.abort();
            Ok(value)
        }
        Err(err) => {
            browser.close().await.ok();
            handle.abort();
            Err(err)
        }
    }
}

#[cfg(test)]
mod storage_value_tests {
    use super::normalize_storage_value;

    #[test]
    fn empty_string_is_not_captured() {
        assert_eq!(
            normalize_storage_value(serde_json::Value::String("".into())),
            None
        );
        assert_eq!(
            normalize_storage_value(serde_json::Value::String("   ".into())),
            None
        );
    }

    #[test]
    fn null_is_not_captured() {
        assert_eq!(normalize_storage_value(serde_json::Value::Null), None);
    }

    #[test]
    fn non_empty_string_is_captured() {
        assert_eq!(
            normalize_storage_value(serde_json::Value::String("tok".into())),
            Some("tok".into())
        );
    }
}
