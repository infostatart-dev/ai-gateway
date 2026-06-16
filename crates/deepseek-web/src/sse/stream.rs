use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    collect::{CollectedSse, SseParser},
    model::{SseDelta, append_search_citations},
};

pub fn transform_sse_to_openai(raw: &str, model: &str) -> Vec<u8> {
    let stream_model = if model.trim().is_empty() {
        "deepseek-web"
    } else {
        model
    };
    let id = format!(
        "chatcmpl-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        &Uuid::new_v4().simple().to_string()[..8]
    );
    let created = chrono::Utc::now().timestamp();
    let mut out = String::new();
    let mut parser = SseParser::new(stream_model);
    let mut emitted_role = false;
    let mut done = false;

    for line in raw.lines() {
        let payload = match line.strip_prefix("data:") {
            Some(p) => p.trim(),
            None => continue,
        };
        if payload == "[DONE]" {
            done = true;
            break;
        }
        for delta in parser.feed_line(line) {
            ensure_role(
                &mut emitted_role,
                &id,
                created,
                stream_model,
                &mut out,
            );
            emit_delta(&id, created, stream_model, &delta, &mut out);
        }
    }

    if !done {
        for delta in parser.feed_line("data: [DONE]") {
            ensure_role(
                &mut emitted_role,
                &id,
                created,
                stream_model,
                &mut out,
            );
            emit_delta(&id, created, stream_model, &delta, &mut out);
        }
    }

    let citations =
        append_search_citations(parser.search_results(), stream_model);
    if !citations.is_empty() {
        ensure_role(&mut emitted_role, &id, created, stream_model, &mut out);
        emit_chunk(
            &id,
            created,
            stream_model,
            json!({ "content": format!("\n\n{citations}") }),
            None,
            &mut out,
        );
    }

    ensure_role(&mut emitted_role, &id, created, stream_model, &mut out);
    emit_chunk(
        &id,
        created,
        stream_model,
        json!({}),
        Some("stop"),
        &mut out,
    );
    out.push_str("data: [DONE]\n\n");
    out.into_bytes()
}

fn ensure_role(
    emitted: &mut bool,
    id: &str,
    created: i64,
    model: &str,
    out: &mut String,
) {
    if !*emitted {
        *emitted = true;
        emit_chunk(
            id,
            created,
            model,
            json!({ "role": "assistant", "content": "" }),
            None,
            out,
        );
    }
}

fn emit_delta(
    id: &str,
    created: i64,
    model: &str,
    delta: &SseDelta,
    out: &mut String,
) {
    let json_delta = match delta {
        SseDelta::Content(text) => json!({ "content": text }),
        SseDelta::Reasoning(text) => json!({ "reasoning_content": text }),
    };
    emit_chunk(id, created, model, json_delta, None, out);
}

fn emit_chunk(
    id: &str,
    created: i64,
    model: &str,
    delta: Value,
    finish: Option<&str>,
    out: &mut String,
) {
    let chunk = json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{ "index": 0, "delta": delta, "finish_reason": finish }],
    });
    out.push_str("data: ");
    out.push_str(&chunk.to_string());
    out.push_str("\n\n");
}

pub fn build_non_stream_response(
    model: &str,
    collected: &CollectedSse,
) -> Value {
    let created = chrono::Utc::now().timestamp();
    let mut message =
        json!({ "role": "assistant", "content": collected.content });
    if !collected.reasoning_content.is_empty() {
        message["reasoning_content"] = json!(collected.reasoning_content);
    }
    json!({
        "id": format!("chatcmpl-{}", Uuid::new_v4()),
        "object": "chat.completion",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": "stop",
        }],
        "usage": { "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 },
    })
}
