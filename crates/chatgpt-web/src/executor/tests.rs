use serde_json::json;
use std::sync::Arc;

use crate::{
    executor::{ExecuteRequest, Executor},
    sentinel::dpl::clear_dpl_cache,
    session::exchange::{clear_all_token_cache, invalidate_token_cache},
    tls::fetch::{FetchResponse, MockFetch},
};

fn session_resp() -> FetchResponse {
    FetchResponse {
        status: 200,
        headers: vec![],
        body: br#"{"accessToken":"tok","user":{"id":"u1"}}"#.to_vec(),
    }
}

fn dpl_resp() -> FetchResponse {
    FetchResponse {
        status: 200,
        headers: vec![],
        body: br#"<html data-build="build123"></html>"#.to_vec(),
    }
}

fn prepare_resp() -> FetchResponse {
    FetchResponse {
        status: 200,
        headers: vec![],
        body: br#"{"prepare_token":"pt"}"#.to_vec(),
    }
}

fn cr_resp() -> FetchResponse {
    FetchResponse {
        status: 200,
        headers: vec![],
        body: br#"{"token":"st","proofofwork":{"required":false}}"#.to_vec(),
    }
}

fn conv_sse(content: &str) -> FetchResponse {
    conv_sse_meta(content, "conv-1", "msg-1")
}

fn conv_sse_meta(content: &str, conv_id: &str, msg_id: &str) -> FetchResponse {
    let escaped = content.replace('\\', "\\\\").replace('"', "\\\"");
    let body = format!(
        "data: {{\"conversation_id\":\"{conv_id}\",\"message\":{{\"id\":\"\
         {msg_id}\",\"author\":{{\"role\":\"assistant\"}},\"content\":{{\"parts\
         \":[\"{escaped}\"]}},\"status\":\"finished_successfully\"}}}}\n\ndata: \
         [DONE]\n\n"
    );
    FetchResponse {
        status: 200,
        headers: vec![],
        body: body.into_bytes(),
    }
}

fn warmup_resp() -> FetchResponse {
    FetchResponse {
        status: 200,
        headers: vec![],
        body: br#"{}"#.to_vec(),
    }
}

fn warmup_responses() -> Vec<FetchResponse> {
    vec![warmup_resp(), warmup_resp(), warmup_resp()]
}

fn execute_once_responses(conv: FetchResponse) -> Vec<FetchResponse> {
    let mut responses = warmup_responses();
    responses.push(prepare_resp());
    responses.push(cr_resp());
    responses.push(conv);
    responses
}

fn first_execute_once_responses(conv: FetchResponse) -> Vec<FetchResponse> {
    let mut responses = vec![session_resp(), dpl_resp()];
    responses.extend(execute_once_responses(conv));
    responses
}

fn strict_schema_body() -> serde_json::Value {
    json!({
        "model": "gpt-5-mini",
        "messages": [{ "role": "user", "content": "extract" }],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "out",
                "strict": true,
                "schema": {
                    "type": "object",
                    "properties": { "status": { "type": "string" } },
                    "required": ["status"],
                    "additionalProperties": false
                }
            }
        }
    })
}

fn reset_caches(cookie: &str) {
    clear_dpl_cache();
    clear_all_token_cache();
    invalidate_token_cache(cookie);
}

#[tokio::test]
#[serial_test::serial]
async fn retries_until_assistant_content_is_valid_json() {
    let cookie = "executor-json-retry-ok";
    reset_caches(cookie);
    let fetch = MockFetch::new({
        let mut responses =
            first_execute_once_responses(conv_sse("Sure! Here is your JSON."));
        responses
            .extend(execute_once_responses(conv_sse(r#"{"status":"ok"}"#)));
        responses
    });
    let exec = Executor::new(fetch);
    let result = exec
        .execute(ExecuteRequest {
            cookie: cookie.into(),
            body: strict_schema_body(),
            json_schema_required: true,
            session_path: None,
        })
        .await
        .unwrap();
    assert_eq!(result.status, 200);
    let value: serde_json::Value =
        serde_json::from_slice(&result.body).unwrap();
    assert!(
        crate::schema::check_structured_response(
            &value,
            crate::schema::parse_json_schema_spec(&strict_schema_body())
                .as_ref(),
        )
        .is_none()
    );
}

#[tokio::test]
#[serial_test::serial]
async fn retries_when_json_valid_but_schema_mismatch() {
    let cookie = "executor-schema-retry-ok";
    reset_caches(cookie);
    let fetch = MockFetch::new({
        let mut responses =
            first_execute_once_responses(conv_sse(r#"{"status":42}"#));
        responses
            .extend(execute_once_responses(conv_sse(r#"{"status":"ok"}"#)));
        responses
    });
    let exec = Executor::new(fetch);
    let result = exec
        .execute(ExecuteRequest {
            cookie: cookie.into(),
            body: strict_schema_body(),
            json_schema_required: true,
            session_path: None,
        })
        .await
        .unwrap();
    assert_eq!(result.status, 200);
}

#[tokio::test]
#[serial_test::serial]
async fn returns_502_after_schema_retries_exhausted() {
    let cookie = "executor-schema-retry-fail";
    reset_caches(cookie);
    let bad = conv_sse(r#"{"status":42}"#);
    let fetch = MockFetch::new({
        let mut responses = first_execute_once_responses(bad.clone());
        responses.extend(execute_once_responses(bad.clone()));
        responses.extend(execute_once_responses(bad));
        responses
    });
    let exec = Executor::new(fetch);
    let result = exec
        .execute(ExecuteRequest {
            cookie: cookie.into(),
            body: strict_schema_body(),
            json_schema_required: true,
            session_path: None,
        })
        .await
        .unwrap();
    assert_eq!(result.status, 502);
    let value: serde_json::Value =
        serde_json::from_slice(&result.body).unwrap();
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("schema")
    );
}

#[tokio::test]
#[serial_test::serial]
async fn returns_502_after_json_retries_exhausted() {
    let cookie = "executor-json-retry-fail";
    reset_caches(cookie);
    let bad = conv_sse("Still prose, not JSON.");
    let fetch = MockFetch::new({
        let mut responses = first_execute_once_responses(bad.clone());
        responses.extend(execute_once_responses(bad.clone()));
        responses.extend(execute_once_responses(bad));
        responses
    });
    let exec = Executor::new(fetch);
    let result = exec
        .execute(ExecuteRequest {
            cookie: cookie.into(),
            body: strict_schema_body(),
            json_schema_required: true,
            session_path: None,
        })
        .await
        .unwrap();
    assert_eq!(result.status, 502);
    let value: serde_json::Value =
        serde_json::from_slice(&result.body).unwrap();
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not valid JSON")
    );
}

#[tokio::test]
#[serial_test::serial]
async fn uploads_oversized_context_in_multiple_turns_before_final_json() {
    let cookie = "executor-chunk-upload";
    reset_caches(cookie);
    let dossier = "word ".repeat(76_000);
    let mut body = strict_schema_body();
    body["messages"] = json!([{ "role": "user", "content": dossier }]);
    let fetch = MockFetch::new({
        let mut responses = first_execute_once_responses(conv_sse_meta(
            "OK",
            "conv-1",
            "upload-msg",
        ));
        responses.push(conv_sse(r#"{"status":"ok"}"#));
        responses
    });
    let fetch_for_count = Arc::clone(&fetch);
    let exec = Executor::new(fetch);
    let result = exec
        .execute(ExecuteRequest {
            cookie: cookie.into(),
            body,
            json_schema_required: true,
            session_path: None,
        })
        .await
        .unwrap();
    assert_eq!(result.status, 200);
    assert!(fetch_for_count.call_count() > 7);
}

#[tokio::test]
#[serial_test::serial]
async fn skips_json_validation_when_not_required() {
    let cookie = "executor-json-skip";
    reset_caches(cookie);
    let fetch = MockFetch::new(first_execute_once_responses(conv_sse(
        "plain text answer",
    )));
    let exec = Executor::new(fetch);
    let result = exec
        .execute(ExecuteRequest {
            cookie: cookie.into(),
            body: json!({
                "model": "gpt-5-mini",
                "messages": [{ "role": "user", "content": "hi" }]
            }),
            json_schema_required: false,
            session_path: None,
        })
        .await
        .unwrap();
    assert_eq!(result.status, 200);
}
