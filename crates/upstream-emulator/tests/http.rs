use upstream_emulator::{EmulatorConfig, SharedState, bind_ephemeral};

#[tokio::test]
async fn health_endpoint() {
    let (addr, handle) = bind_ephemeral().await.expect("bind");
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let body = client
        .get(format!("{base}/health"))
        .send()
        .await
        .expect("health")
        .text()
        .await
        .expect("body");
    assert_eq!(body, "ok");
    handle.abort();
}

#[tokio::test]
async fn any_api_key_provider_mount_returns_ok() {
    let state = SharedState::new(EmulatorConfig {
        default_latency_ms: 0,
        ..EmulatorConfig::default()
    });
    let provider = state
        .table
        .entries
        .first()
        .expect("catalog has api providers")
        .id
        .to_string();
    let (addr, handle) = bind_ephemeral().await.expect("bind");
    let url = format!("http://{addr}/{provider}/v1/chat/completions");
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", "Bearer emu-key")
        .json(&serde_json::json!({
            "model": "default",
            "messages": [{ "role": "user", "content": "ping" }]
        }))
        .send()
        .await
        .expect("post");
    assert!(response.status().is_success());
    let json: serde_json::Value = response.json().await.expect("json");
    let content = json
        .pointer("/choices/0/message/content")
        .or_else(|| json.pointer("/content/0/text"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    assert_eq!(content, "ok");
    handle.abort();
}

#[tokio::test]
async fn per_credential_rpm_isolation_on_http_mount() {
    let (addr, handle) = bind_ephemeral().await.expect("bind");
    let url = format!("http://{addr}/groq/openai/v1/chat/completions");
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": "llama-3.1-8b-instant",
        "messages": [{ "role": "user", "content": "x" }]
    });
    for _ in 0..30 {
        let status = client
            .post(&url)
            .header("Authorization", "Bearer key-a")
            .json(&body)
            .send()
            .await
            .expect("post")
            .status();
        assert!(status.is_success() || status.as_u16() == 429);
    }
    let blocked = client
        .post(&url)
        .header("Authorization", "Bearer key-a")
        .json(&body)
        .send()
        .await
        .expect("post")
        .status();
    assert_eq!(blocked.as_u16(), 429);
    let sibling = client
        .post(&url)
        .header("Authorization", "Bearer key-b")
        .json(&body)
        .send()
        .await
        .expect("post")
        .status();
    assert!(sibling.is_success());
    handle.abort();
}

#[tokio::test]
async fn fat_body_returns_large_prompt_tokens() {
    let (addr, handle) = bind_ephemeral().await.expect("bind");
    let url = format!("http://{addr}/groq/openai/v1/chat/completions");
    let client = reqwest::Client::new();
    let filler = "x".repeat(12_000);
    let body = serde_json::json!({
        "model": "llama-3.1-8b-instant",
        "messages": [{"role": "user", "content": filler}]
    });
    let response = client
        .post(&url)
        .header("Authorization", "Bearer emu-groq-default")
        .json(&body)
        .send()
        .await
        .expect("post");
    assert!(response.status().is_success());
    let json: serde_json::Value = response.json().await.expect("json");
    let prompt = json
        .pointer("/usage/prompt_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    assert!(prompt > 1000, "expected fat prompt_tokens, got {prompt}");
    handle.abort();
}

#[tokio::test]
async fn rate_limit_returns_json_body() {
    let (addr, handle) = bind_ephemeral().await.expect("bind");
    let url = format!("http://{addr}/groq/openai/v1/chat/completions");
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": "llama-3.1-8b-instant",
        "messages": [{ "role": "user", "content": "x" }]
    });
    for _ in 0..31 {
        let _ = client
            .post(&url)
            .header("Authorization", "Bearer key-a")
            .json(&body)
            .send()
            .await;
    }
    let response = client
        .post(&url)
        .header("Authorization", "Bearer key-a")
        .json(&body)
        .send()
        .await
        .expect("post");
    assert_eq!(response.status(), 429);
    let json: serde_json::Value = response.json().await.expect("json 429");
    assert!(json.get("error").is_some());
    handle.abort();
}

#[tokio::test]
async fn forced_never_purchased_profile_returns_402() {
    let (addr, handle) = bind_ephemeral().await.expect("bind");
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let scope = "openrouter:emu-openrouter-default";
    client
        .post(format!("{base}/_admin/profile"))
        .json(&serde_json::json!({
            "scope": scope,
            "action": "402-never-purchased"
        }))
        .send()
        .await
        .expect("set profile");
    let response = client
        .post(format!("{base}/openrouter/openai/v1/chat/completions"))
        .header("Authorization", "Bearer emu-openrouter-default")
        .json(&serde_json::json!({
            "model": "openai/gpt-4o-mini",
            "messages": [{"role": "user", "content": "hi"}]
        }))
        .send()
        .await
        .expect("post");
    assert_eq!(response.status(), 402);
    handle.abort();
}

#[tokio::test]
async fn forced_free_models_per_day_profile_returns_429_with_reset() {
    let (addr, handle) = bind_ephemeral().await.expect("bind");
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let scope = "openrouter:emu-openrouter-default";
    client
        .post(format!("{base}/_admin/profile"))
        .json(&serde_json::json!({
            "scope": scope,
            "action": "429-free-models-per-day"
        }))
        .send()
        .await
        .expect("set profile");
    let response = client
        .post(format!("{base}/openrouter/openai/v1/chat/completions"))
        .header("Authorization", "Bearer emu-openrouter-default")
        .json(&serde_json::json!({
            "model": "nvidia/nemotron-3-nano-30b-a3b:free",
            "messages": [{"role": "user", "content": "hi"}]
        }))
        .send()
        .await
        .expect("post");
    assert_eq!(response.status(), 429);
    assert!(response.headers().contains_key("x-ratelimit-reset"));
    handle.abort();
}
