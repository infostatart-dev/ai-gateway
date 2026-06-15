use chatgpt_web::{
    session::{
        build_session_cookie_header, exchange_session, file::load_session,
        warmup::run_session_warmup,
    },
    tls::fetch::{FetchRequest, HttpFetch, RquestFetch},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path =
        std::env::var("CHATGPT_BROWSER_CLI").expect("CHATGPT_BROWSER_CLI");
    let session = load_session(path.as_ref()).await?;
    let cookie = session.normalized_cookie();
    let fetch = RquestFetch;
    let token = exchange_session(&fetch, &cookie).await?;
    let session_id = uuid::Uuid::new_v4().to_string();
    let device_id = "00000000-0000-4000-8000-000000000001".to_string();
    run_session_warmup(
        &fetch,
        &token.access_token,
        token.account_id.as_deref(),
        &session_id,
        &device_id,
        &cookie,
    )
    .await;

    let mut headers = chatgpt_web::headers::browser_headers();
    headers.extend(chatgpt_web::headers::oai_headers(&session_id, &device_id));
    headers.push(("Accept".into(), "application/json".into()));
    headers.push((
        "Authorization".into(),
        format!("Bearer {}", token.access_token),
    ));
    headers.push(("Cookie".into(), build_session_cookie_header(&cookie)));
    if let Some(id) = &token.account_id {
        headers.push(("chatgpt-account-id".into(), id.clone()));
    }

    let resp = fetch
        .fetch(FetchRequest {
            url: "https://chatgpt.com/backend-api/models?history_and_training_disabled=false"
                .into(),
            method: "GET".into(),
            headers,
            body: None,
            timeout_ms: 30_000,
        })
        .await?;
    println!("status {}", resp.status);
    let text = String::from_utf8_lossy(&resp.body);
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
        if let Some(models) = v.get("models").and_then(|m| m.as_array()) {
            for m in models.iter().take(20) {
                let slug =
                    m.get("slug").and_then(|s| s.as_str()).unwrap_or("?");
                let title = m
                    .get("title")
                    .or_else(|| m.get("display_name"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("?");
                println!("{slug}\t{title}");
            }
        } else {
            println!("{text}");
        }
    } else {
        println!("{text}");
    }
    Ok(())
}
