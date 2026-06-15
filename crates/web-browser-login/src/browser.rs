use std::path::{Path, PathBuf};

use chromiumoxide::browser::BrowserConfig;

/// Same path chromiumoxide uses when `user_data_dir` is unset.
pub fn default_user_data_dir() -> PathBuf {
    std::env::temp_dir().join("chromiumoxide-runner")
}

pub fn wipe_user_data_dir(path: &Path) -> Result<(), String> {
    if path.exists() {
        std::fs::remove_dir_all(path).map_err(|e| {
            format!("failed to wipe browser profile {}: {e}", path.display())
        })?;
    }
    Ok(())
}

pub fn system_chrome_executable() -> Option<PathBuf> {
    const CANDIDATES: &[&str] = &[
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
    ];
    CANDIDATES
        .iter()
        .find(|p| Path::new(p).exists())
        .map(|p| PathBuf::from(*p))
}

pub fn browser_config(
    user_data_dir: Option<PathBuf>,
    chrome_executable: Option<PathBuf>,
) -> Result<BrowserConfig, String> {
    let data_dir = user_data_dir.unwrap_or_else(default_user_data_dir);
    wipe_user_data_dir(&data_dir)?;
    eprintln!("Fresh browser profile → {}", data_dir.display());

    let chrome = chrome_executable.or_else(system_chrome_executable);
    if let Some(ref exe) = chrome {
        eprintln!("Browser binary → {}", exe.display());
    }

    // Drop chromiumoxide defaults — they include `--enable-automation`, which
    // Cloudflare Turnstile / PAT detects and loops forever on perplexity.ai.
    let mut builder = BrowserConfig::builder()
        .with_head()
        .viewport(None)
        .window_size(1280, 800)
        .user_data_dir(&data_dir)
        .disable_default_args()
        .arg("--disable-blink-features=AutomationControlled")
        .arg("--exclude-switches=enable-automation")
        .arg("--disable-dev-shm-usage")
        .arg("--no-first-run")
        .arg("--no-default-browser-check");
    if let Some(exe) = chrome {
        builder = builder.chrome_executable(exe);
    }
    builder.build().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wipe_removes_existing_dir() {
        let path = std::env::temp_dir().join("web-browser-login-wipe-test");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("marker"), "x").unwrap();
        wipe_user_data_dir(&path).unwrap();
        assert!(!path.exists());
    }
}
