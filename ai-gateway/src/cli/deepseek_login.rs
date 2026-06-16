//! CLI: `deepseek login | import | probe`.

use crate::config::deepseek_web as deepseek_cfg;

pub async fn run_login() -> Result<(), Box<dyn std::error::Error>> {
    let path = deepseek_cfg::default_session_path();
    deepseek_web::login::run_login_to(&path).await?;
    Ok(())
}

pub async fn run_import(
    token: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = deepseek_cfg::default_session_path();
    deepseek_web::login::save_session_from_token(&path, token.trim()).await?;
    Ok(())
}

pub async fn run_probe(
    query: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = deepseek_cfg::default_session_path();
    let session = deepseek_web::load_session(&path).await?;
    let user_token = deepseek_web::normalize_user_token(&session.token);
    if user_token.is_empty() {
        return Err("session file has no token — run: deepseek login".into());
    }

    let fetch = deepseek_web::tls::fetch::default_fetch();
    let access = deepseek_web::session::exchange::exchange_session(
        fetch.as_ref(),
        &user_token,
    )
    .await?;
    eprintln!(
        "users/current OK — access token acquired ({} chars)",
        access.token.len()
    );

    if let Some(query) = query {
        use deepseek_web::{ExecuteRequest, Executor};
        use serde_json::json;

        let result = Executor::default()
            .execute(ExecuteRequest {
                user_token,
                body: json!({
                    "model": "deepseek-chat",
                    "stream": false,
                    "messages": [{ "role": "user", "content": query }],
                }),
                stream: false,
            })
            .await?;
        let body: serde_json::Value = serde_json::from_slice(&result.body)?;
        eprintln!("completion status={} body={body}", result.status);
    }

    Ok(())
}
