use serde_json::Value;
use web_message_budget::{
    CHATGPT_UPLOAD_PAYLOAD_TOKENS, CHATGPT_WEB_CONTEXT_TOKENS, ChunkPlan,
    MessageBudget, ParsedChat, plan_web_chunks,
};
pub use web_message_budget::{
    ParsedChat as ParsedMessages, parse_openai_messages,
};

pub fn build_conversation_body(
    parsed: &ParsedMessages,
    model_slug: &str,
    thinking_effort: Option<&str>,
    parent_message_id: &str,
    conversation_id: Option<&str>,
) -> Value {
    let mut system_parts = Vec::new();
    if !parsed.system_msg.trim().is_empty() {
        system_parts.push(parsed.system_msg.trim().to_string());
    }

    let mut messages = Vec::new();
    if !system_parts.is_empty() {
        messages.push(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "author": { "role": "system" },
            "content": { "content_type": "text", "parts": [system_parts.join("\n\n")] },
        }));
    }
    messages.push(serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "author": { "role": "user" },
        "content": { "content_type": "text", "parts": [parsed.current_msg] },
    }));

    let mut body = serde_json::json!({
        "action": "next",
        "messages": messages,
        "model": model_slug,
        "conversation_id": conversation_id,
        "parent_message_id": parent_message_id,
        "timezone_offset_min": chrono::Local::now().offset().local_minus_utc() / 60,
        "history_and_training_disabled": true,
        "suggestions": [],
        "websocket_request_id": uuid::Uuid::new_v4().to_string(),
    });
    if let Some(effort) = thinking_effort {
        body["thinking_effort"] = Value::String(effort.to_string());
    }
    body
}

#[must_use]
pub fn plan_conversation_turns(
    parsed: &ParsedChat,
    base_system: &str,
    schema_instruction: Option<&str>,
    reserved_output_tokens: u32,
) -> ChunkPlan {
    plan_web_chunks(
        parsed,
        base_system,
        schema_instruction,
        MessageBudget {
            max_context_tokens: CHATGPT_WEB_CONTEXT_TOKENS,
            reserved_output_tokens,
            upload_part_token_cap: CHATGPT_UPLOAD_PAYLOAD_TOKENS,
            ..MessageBudget::default()
        },
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use web_message_budget::WebTurnKind;

    use super::*;
    use crate::schema::{build_schema_instruction, parse_json_schema_spec};

    #[test]
    fn single_turn_when_payload_small() {
        let parsed =
            parse_openai_messages(&[json!({"role":"user","content":"hi"})]);
        let plan = plan_conversation_turns(&parsed, "", None, 4_096);
        assert_eq!(plan.turns.len(), 1);
    }

    #[test]
    fn huge_dossier_splits_into_uploads_not_truncation() {
        let dossier = "word ".repeat(157_000 * 3);
        let parsed =
            parse_openai_messages(&[json!({"role":"user","content":dossier})]);
        let plan = plan_conversation_turns(&parsed, "", None, 4_096);
        assert!(plan.turns.len() > 1);
        assert!(matches!(
            plan.turns[0].kind,
            WebTurnKind::ContextUpload { part: 1, .. }
        ));
        assert!(matches!(
            plan.turns.last().unwrap().kind,
            WebTurnKind::Final
        ));
        let joined: String =
            plan.turns.iter().map(|t| t.user_msg.clone()).collect();
        assert!(joined.contains("word "));
        assert!(!joined.contains("truncated"));
    }

    #[test]
    fn strict_schema_only_on_final_turn() {
        let body = json!({
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "out",
                    "strict": true,
                    "schema": { "type": "object" }
                }
            }
        });
        let schema =
            parse_json_schema_spec(&body).map(|s| build_schema_instruction(&s));
        let huge = "word ".repeat(400_000 * 3);
        let parsed = parse_openai_messages(&[
            json!({"role":"system","content":"base rules"}),
            json!({"role":"user","content":huge}),
        ]);
        let plan = plan_conversation_turns(
            &parsed,
            "base rules",
            schema.as_deref(),
            4_096,
        );
        assert!(plan.turns.len() >= 2);
        assert!(!plan.turns[0].system_msg.contains("MANDATORY strict mode"));
        assert!(
            plan.turns
                .last()
                .unwrap()
                .system_msg
                .contains("MANDATORY strict mode")
        );
    }
}
