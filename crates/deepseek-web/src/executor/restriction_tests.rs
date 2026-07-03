//! Executor restriction behavior (structured-output guard + mute path).

use std::collections::BTreeMap;

use serde_json::json;

use crate::{
    Error,
    executor::{ExecuteRequest, Executor},
    session::{exchange::clear_token_cache, file::BrowserSession},
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

fn header_value<'a>(
    headers: &'a [(String, String)],
    name: &str,
) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
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
            browser_session: None,
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

#[tokio::test]
async fn browser_session_cookie_and_headers_reach_all_deepseek_calls() {
    clear_token_cache();
    let fetch = MockFetch::new(vec![
        exchange_response(),
        session_response(),
        pow_challenge_response(),
        answer_sse("ok"),
        json_response(json!({ "code": 0 })),
    ]);
    let executor = Executor::new(fetch.clone());
    let cookie = "cf_clearance=clear; ds_session_id=session; aws-waf-token=waf";
    let browser_session = BrowserSession {
        token: "browser-session-user-token-12345678".into(),
        cookie: Some(cookie.into()),
        headers: BTreeMap::from([
            ("user-agent".into(), "Mozilla/5.0 Chrome/149".into()),
            ("x-app-version".into(), "2.0.0".into()),
            ("x-client-bundle-id".into(), "com.deepseek.chat".into()),
            ("x-client-locale".into(), "ru".into()),
            ("x-client-platform".into(), "web".into()),
            ("x-client-timezone-offset".into(), "10800".into()),
            ("x-client-version".into(), "2.0.0".into()),
        ]),
    };

    let result = executor
        .execute(ExecuteRequest {
            user_token: browser_session.token.clone(),
            browser_session: Some(browser_session),
            body: json!({
                "model": "deepseek-chat",
                "messages": [{ "role": "user", "content": "hi" }]
            }),
            stream: false,
            turn_hook: None,
        })
        .await
        .expect("deepseek execution");
    assert_eq!(result.status, 200);

    let requests = fetch.requests();
    assert_eq!(
        requests.len(),
        5,
        "exchange + session + pow + completion + delete"
    );
    for request in &requests {
        assert_eq!(header_value(&request.headers, "Cookie"), Some(cookie));
        assert_eq!(
            header_value(&request.headers, "X-Client-Version"),
            Some("2.0.0")
        );
        assert_eq!(
            header_value(&request.headers, "X-Client-Bundle-Id"),
            Some("com.deepseek.chat")
        );
        assert_eq!(
            header_value(&request.headers, "User-Agent"),
            Some("Mozilla/5.0 Chrome/149")
        );
    }
}
