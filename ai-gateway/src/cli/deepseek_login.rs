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
    structured_output: bool,
    context_limit: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if structured_output && context_limit {
        return Err("choose only one of --structured-output or \
                    --context-limit"
            .into());
    }
    if structured_output {
        return run_probe_structured_output().await;
    }
    if context_limit {
        return run_probe_context_limit().await;
    }

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
                turn_hook: None,
            })
            .await?;
        let body: serde_json::Value = serde_json::from_slice(&result.body)?;
        eprintln!("completion status={} body={body}", result.status);
    }

    Ok(())
}

async fn run_probe_structured_output() -> Result<(), Box<dyn std::error::Error>>
{
    use deepseek_web::{ExecuteRequest, Executor};
    use serde_json::json;
    use web_structured_output::{
        check_structured_response, parse_json_schema_spec,
    };

    let path = deepseek_cfg::default_session_path();
    let session = deepseek_web::load_session(&path).await?;
    let user_token = deepseek_web::normalize_user_token(&session.token);
    if user_token.is_empty() {
        return Err("session file has no token — run: deepseek login".into());
    }

    let body = json!({
        "model": "deepseek-chat",
        "stream": false,
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "probe",
                "strict": true,
                "schema": {
                    "type": "object",
                    "properties": { "status": { "type": "string" } },
                    "required": ["status"],
                    "additionalProperties": false
                }
            }
        },
        "messages": [{ "role": "user", "content": "Reply with status ok." }]
    });

    let result = Executor::default()
        .execute(ExecuteRequest {
            user_token,
            body: body.clone(),
            stream: false,
            turn_hook: None,
        })
        .await?;
    let response: serde_json::Value = serde_json::from_slice(&result.body)?;
    let spec = parse_json_schema_spec(&body);
    if check_structured_response(&response, spec.as_ref()).is_none() {
        eprintln!("structured-output probe: PASS");
        Ok(())
    } else {
        eprintln!("structured-output probe: FAIL body={response}");
        std::process::exit(1);
    }
}

async fn run_probe_context_limit() -> Result<(), Box<dyn std::error::Error>> {
    use deepseek_web::{ExecuteRequest, Executor};
    use serde_json::json;

    let path = deepseek_cfg::default_session_path();
    let session = deepseek_web::load_session(&path).await?;
    let user_token = deepseek_web::normalize_user_token(&session.token);
    if user_token.is_empty() {
        return Err("session file has no token — run: deepseek login".into());
    }

    let sizes = [
        8_000usize, 16_000, 32_000, 64_000, 96_000, 128_000, 160_000, 200_000,
    ];
    let mut last_ok = 0usize;
    for words in sizes {
        let prompt = "word ".repeat(words * 2);
        let result = Executor::default()
            .execute(ExecuteRequest {
                user_token: user_token.clone(),
                body: json!({
                    "model": "deepseek-chat",
                    "stream": false,
                    "messages": [{ "role": "user", "content": prompt }],
                }),
                stream: false,
                turn_hook: None,
            })
            .await;
        match result {
            Ok(r) if r.status == 200 => {
                last_ok = words * 2;
                eprintln!("context-limit probe: {words} word-units OK");
            }
            Ok(r) => {
                eprintln!(
                    "context-limit probe: failed at ~{words} word-units (HTTP \
                     {})",
                    r.status
                );
                break;
            }
            Err(e) => {
                eprintln!(
                    "context-limit probe: failed at ~{words} word-units ({e})"
                );
                break;
            }
        }
    }
    eprintln!(
        "context-limit probe: largest successful single-prompt ~{last_ok} \
         words; recommended catalog context-window: 128000 unless probe \
         exceeds it"
    );
    Ok(())
}
