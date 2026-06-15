use serde_json::Value;
use web_message_budget::{
    CHATGPT_WEB_CONTEXT_TOKENS, ChunkPlan, MessageBudget, ParsedChat,
    parse_openai_messages, plan_web_chunks,
};

/// Build Perplexity wire query for one turn of a chunk plan.
#[must_use]
pub fn build_turn_query(turn: &web_message_budget::WebTurn) -> String {
    turn.user_msg.clone()
}

/// Multi-turn chunk plan for Perplexity web (same semantics as ChatGPT web).
#[must_use]
pub fn plan_perplexity_turns(
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
            ..MessageBudget::default()
        },
    )
}

/// Parse OpenAI messages and build upload/final turn plan.
#[must_use]
pub fn prepare_turn_plan_from_messages(
    messages: &[Value],
    base_system: &str,
    schema_instruction: Option<&str>,
    reserved_output_tokens: u32,
) -> ChunkPlan {
    let parsed = parse_openai_messages(messages);
    plan_perplexity_turns(
        &parsed,
        base_system,
        schema_instruction,
        reserved_output_tokens,
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use web_message_budget::WebTurnKind;

    use super::*;

    #[test]
    fn first_message_wraps_context_upload_header() {
        let parsed =
            parse_openai_messages(&[json!({"role":"user","content":"hello"})]);
        let plan = plan_perplexity_turns(&parsed, "", None, 4_096);
        assert_eq!(plan.turns.len(), 1);
        assert!(matches!(plan.turns[0].kind, WebTurnKind::Final));
    }

    #[test]
    fn huge_dossier_uses_multi_turn_uploads() {
        let huge = "word ".repeat(400_000 * 3);
        let plan = prepare_turn_plan_from_messages(
            &[json!({"role":"user","content":huge})],
            "",
            None,
            4_096,
        );
        assert!(plan.turns.len() > 1);
        assert!(plan.turns[0].user_msg.starts_with("[Context part 1/"));
        assert!(!plan.turns[0].user_msg.contains("truncated"));
    }

    #[test]
    fn strict_schema_only_on_final_perplexity_turn() {
        let schema = "MANDATORY strict mode";
        let huge = "a".repeat(400_000 * 3);
        let plan = prepare_turn_plan_from_messages(
            &[json!({"role":"user","content":huge})],
            "base",
            Some(schema),
            4_096,
        );
        assert!(!plan.turns[0].system_msg.contains("MANDATORY"));
        assert!(plan.turns.last().unwrap().system_msg.contains("MANDATORY"));
    }

    #[test]
    fn upload_header_format() {
        assert!(web_message_budget::upload_part_header(2, 5).contains("2/5"));
    }
}
