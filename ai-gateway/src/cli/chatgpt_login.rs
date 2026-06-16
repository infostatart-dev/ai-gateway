//! CLI entry for `ai-gateway chatgpt login|import`.

use crate::config::chatgpt_web as chatgpt_cfg;

pub async fn run_login() -> Result<(), Box<dyn std::error::Error>> {
    let path = chatgpt_cfg::default_session_path();
    chatgpt_web::login::run_login_to(&path).await?;
    Ok(())
}

pub async fn run_import(
    cookie: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = chatgpt_cfg::default_session_path();
    chatgpt_web::login::save_session_from_cookie(&path, cookie.trim()).await?;
    Ok(())
}
