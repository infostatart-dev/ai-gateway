use crate::{
    token::estimate_tokens,
    types::{ChunkPlan, MessageBudget, ParsedChat, WebTurn, WebTurnKind},
};

const UPLOAD_PAYLOAD_TOKENS: usize = 90_000;
const UPLOAD_ACK_SYSTEM: &str = "The user will upload context in several messages. \
    Acknowledge each part with only OK until the final message.";

pub fn upload_part_header(part: usize, total: usize) -> String {
    format!(
        "[Context part {part}/{total}] Store this — more context follows in the \
         next message. Reply only OK.\n\n"
    )
}

pub fn final_user_suffix() -> &'static str {
    "\n\n[All context parts delivered. Follow the system instructions now.]"
}

/// Multi-turn upload plan for web providers (no truncation).
#[must_use]
pub fn plan_web_chunks(
    parsed: &ParsedChat,
    base_system: &str,
    schema_instruction: Option<&str>,
    budget: MessageBudget,
) -> ChunkPlan {
    let payload = materialize_payload(parsed);
    let payload_tokens = estimate_tokens(&payload);
    let schema_tokens = schema_instruction.map(estimate_tokens).unwrap_or(0);
    let base_tokens = estimate_tokens(base_system);
    let input_budget = budget.input_token_budget() as usize;

    let single_turn_cost =
        payload_tokens + schema_tokens + base_tokens + turn_overhead();
    if single_turn_cost <= input_budget {
        return single_final_turn(parsed, base_system, schema_instruction);
    }

    let parts = split_by_tokens(&payload, UPLOAD_PAYLOAD_TOKENS);
    let total = parts.len();
    let mut turns = Vec::with_capacity(total);

    for (idx, part) in parts.iter().enumerate() {
        let part_no = idx + 1;
        let is_last = part_no == total;
        if is_last {
            let mut system = String::new();
            if !base_system.trim().is_empty() {
                system.push_str(base_system.trim());
            }
            if let Some(schema) = schema_instruction {
                if !system.is_empty() {
                    system.push_str("\n\n");
                }
                system.push_str(schema);
            }
            let mut user = upload_part_header(part_no, total);
            user.push_str(part);
            if total > 1 {
                user.push_str(final_user_suffix());
            }
            turns.push(WebTurn {
                kind: WebTurnKind::Final,
                system_msg: system,
                user_msg: user,
            });
        } else {
            let system = if idx == 0 {
                UPLOAD_ACK_SYSTEM.to_string()
            } else {
                String::new()
            };
            let mut user = upload_part_header(part_no, total);
            user.push_str(part);
            turns.push(WebTurn {
                kind: WebTurnKind::ContextUpload {
                    part: part_no,
                    total,
                },
                system_msg: system,
                user_msg: user,
            });
        }
    }
    ChunkPlan { turns }
}

fn single_final_turn(
    parsed: &ParsedChat,
    base_system: &str,
    schema_instruction: Option<&str>,
) -> ChunkPlan {
    let mut system = String::new();
    if !base_system.trim().is_empty() {
        system.push_str(base_system.trim());
    }
    if let Some(schema) = schema_instruction {
        if !system.is_empty() {
            system.push_str("\n\n");
        }
        system.push_str(schema);
    }
    ChunkPlan {
        turns: vec![WebTurn {
            kind: WebTurnKind::Final,
            system_msg: system,
            user_msg: parsed.current_msg.clone(),
        }],
    }
}

fn materialize_payload(parsed: &ParsedChat) -> String {
    let mut out = String::new();
    if !parsed.history.is_empty() {
        let hist = parsed
            .history
            .iter()
            .map(|(role, content)| {
                if role == "assistant" {
                    format!("Assistant: {content}")
                } else {
                    format!("User: {content}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        out.push_str("Prior conversation:\n\n");
        out.push_str(&hist);
    }
    if !parsed.current_msg.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&parsed.current_msg);
    }
    out
}

fn split_by_tokens(text: &str, max_tokens: usize) -> Vec<String> {
    let max_chars = max_tokens.saturating_mul(crate::token::CHARS_PER_TOKEN);
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return vec![text.to_string()];
    }
    chars
        .chunks(max_chars)
        .map(|chunk| chunk.iter().collect())
        .collect()
}

fn turn_overhead() -> usize {
    256
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MessageBudget, WebTurnKind};

    fn huge_text(target_tokens: usize) -> String {
        "word ".repeat(target_tokens * 3)
    }

    #[test]
    fn small_payload_single_final_turn() {
        let parsed = ParsedChat {
            system_msg: String::new(),
            history: vec![],
            current_msg: "hi".into(),
        };
        let plan = plan_web_chunks(&parsed, "", None, MessageBudget::default());
        assert_eq!(plan.turns.len(), 1);
        assert!(matches!(plan.turns[0].kind, WebTurnKind::Final));
    }

    #[test]
    fn huge_dossier_splits_into_upload_turns() {
        let dossier = huge_text(157_000);
        let parsed = ParsedChat {
            system_msg: String::new(),
            history: vec![],
            current_msg: dossier,
        };
        let plan = plan_web_chunks(&parsed, "", None, MessageBudget::default());
        assert!(plan.turns.len() > 1);
        assert!(matches!(
            plan.turns[0].kind,
            WebTurnKind::ContextUpload { part: 1, .. }
        ));
        assert!(!plan.turns[0].user_msg.contains("truncated"));
    }

    #[test]
    fn strict_schema_only_on_final_turn() {
        let schema = "MANDATORY strict mode: schema block";
        let parsed = ParsedChat {
            system_msg: String::new(),
            history: vec![],
            current_msg: huge_text(120_000),
        };
        let plan = plan_web_chunks(
            &parsed,
            "base",
            Some(schema),
            MessageBudget::default(),
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

#[must_use]
pub fn fits_single_turn(
    parsed: &ParsedChat,
    base_system: &str,
    schema_instruction: Option<&str>,
    budget: MessageBudget,
) -> bool {
    plan_web_chunks(parsed, base_system, schema_instruction, budget)
        .turns
        .len()
        == 1
}
