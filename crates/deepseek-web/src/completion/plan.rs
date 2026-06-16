pub use web_message_budget::parse_openai_messages;
use web_message_budget::{
    ChunkPlan, DEEPSEEK_UPLOAD_PAYLOAD_TOKENS, DEEPSEEK_WEB_CONTEXT_TOKENS,
    MessageBudget, ParsedChat, plan_web_chunks,
};

#[must_use]
pub fn plan_completion_turns(
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
            max_context_tokens: DEEPSEEK_WEB_CONTEXT_TOKENS,
            reserved_output_tokens,
            upload_part_token_cap: DEEPSEEK_UPLOAD_PAYLOAD_TOKENS,
            ..MessageBudget::default()
        },
    )
}
