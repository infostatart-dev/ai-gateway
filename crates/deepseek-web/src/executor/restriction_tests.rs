//! Executor restriction behavior (structured-output guard + mute path).

use serde_json::json;

use crate::{
    Error,
    executor::{ExecuteRequest, Executor},
    session::exchange::clear_token_cache,
    tls::fetch::{FetchResponse, MockFetch},
};

fn json_response(body: serde_json::Value) -> FetchResponse {
    FetchResponse {
        status: 200,
        headers: vec![("content-type".into(), "application/json".into())],
        body: body.to_string().into_bytes(),
    }
}

fn exchange_response() -> FetchResponse {
    json_response(json!({
        "code": 0,
        "data": { "biz_data": { "token": "access-token" } }
    }))
}

fn session_response() -> FetchResponse {
    json_response(json!({
        "code": 0,
        "data": { "biz_data": { "chat_session": { "id": "sess-1" } } }
    }))
}

fn pow_challenge_response() -> FetchResponse {
    json_response(json!({
        "code": 0,
        "data": { "biz_data": { "challenge": {
            "algorithm": "DeepSeekHashV1",
            "challenge": "d1052c4a04fb634e3ac66d36bfeaa583d769839823812090d679b23de6048d6d",
            "salt": "abc",
            "signature": "sig",
            "difficulty": 1000,
            "expire_at": 1_234_567_890_i64,
            "target_path": "/api/v0/chat/completion"
        }}}
    }))
}

fn answer_sse(content: &str) -> FetchResponse {
    FetchResponse {
        status: 200,
        headers: vec![("content-type".into(), "text/event-stream".into())],
        body: format!(
            "data: {{\"p\":\"response/fragments\",\"v\":[{{\"type\":\"ANSWER\"\
             ,\"content\":\"{content}\"}}]}}\ndata: [DONE]\n"
        )
        .into_bytes(),
    }
}

fn mute_json_response() -> FetchResponse {
    json_response(json!({
        "code": 0,
        "data": {
            "biz_code": 5,
            "biz_msg": "user is muted",
            "biz_data": { "mute_until": 1781861651.742 }
        }
    }))
}

#[tokio::test]
async fn credential_restricted_skips_further_structured_retries() {
    clear_token_cache();
    let fetch = MockFetch::new(vec![
        exchange_response(),
        session_response(),
        pow_challenge_response(),
        answer_sse("not-valid-json"),
        pow_challenge_response(),
        mute_json_response(),
        json_response(json!({ "code": 0 })),
    ]);
    let executor = Executor::new(fetch.clone());
    let body = json!({
        "model": "deepseek-chat",
        "messages": [{ "role": "user", "content": "hi" }],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "out",
                "strict": true,
                "schema": {
                    "type": "object",
                    "properties": { "x": { "type": "string" } },
                    "required": ["x"]
                }
            }
        }
    });

    let err = executor
        .execute(ExecuteRequest {
            user_token: "user-token-12345678".into(),
            body,
            stream: false,
            turn_hook: None,
        })
        .await
        .unwrap_err();

    match err {
        Error::CredentialRestricted { .. } => {}
        other => panic!("expected CredentialRestricted, got {other:?}"),
    }
    assert_eq!(
        fetch.call_count(),
        7,
        "exchange + session + 2×(pow+completion) + delete — no third \
         structured retry"
    );
}
