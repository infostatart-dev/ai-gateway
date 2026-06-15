//! CLI: `perplexity login | import | probe`.

pub async fn run_login() -> Result<(), Box<dyn std::error::Error>> {
    perplexity_web::run_login().await?;
    Ok(())
}

pub async fn run_import(cookie: String) -> Result<(), Box<dyn std::error::Error>> {
    let path = perplexity_web::session_path_from_env()
        .ok_or("PERPLEXITY_BROWSER_CLI env var is not set")?;
    perplexity_web::save_session_from_cookie(&path, cookie.trim()).await?;
    Ok(())
}

pub async fn run_probe(query: String) -> Result<(), Box<dyn std::error::Error>> {
    let path = perplexity_web::session_path_from_env()
        .ok_or("PERPLEXITY_BROWSER_CLI is not set")?;
    let session = perplexity_web::load_session(&path).await?;
    let cookie = session.normalized_cookie();
    if !perplexity_web::session::cookie::has_session_token(&cookie) {
        return Err(
            "session file has no login token — run: perplexity login".into(),
        );
    }
    eprintln!(
        "Account session {} (saved {})",
        path.display(),
        session.saved_at
    );
    let result = perplexity_web::probe_query(&query, &cookie).await?;
    eprintln!("HTTP {}", result.status);
    eprintln!("--- answer ---\n{}", result.answer);
    Ok(())
}
