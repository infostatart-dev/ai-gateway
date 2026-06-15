//! CLI entry for `ai-gateway chatgpt login|import`.

pub async fn run_login() -> Result<(), Box<dyn std::error::Error>> {
    chatgpt_web::login::run_login().await?;
    Ok(())
}

pub async fn run_import(
    cookie: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = chatgpt_web::session_path_from_env()
        .ok_or("CHATGPT_BROWSER_CLI env var is not set")?;
    chatgpt_web::login::save_session_from_cookie(&path, cookie.trim()).await?;
    Ok(())
}
