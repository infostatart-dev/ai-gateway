use web_message_budget::{WebTurnKind, parse_openai_messages};
use web_structured_output::{build_schema_instruction, parse_json_schema_spec};

use crate::completion::plan_completion_turns;

#[test]
fn large_dossier_splits_at_45k_parts() {
    let dossier = "word ".repeat(45_000 * 4);
    let parsed = parse_openai_messages(&[serde_json::json!({
        "role": "user",
        "content": dossier
    })]);
    let plan = plan_completion_turns(&parsed, "", None, 4_096);
    assert!(plan.turns.len() > 1);
    assert!(matches!(
        plan.turns[0].kind,
        WebTurnKind::ContextUpload { part: 1, .. }
    ));
    let joined: String =
        plan.turns.iter().map(|t| t.user_msg.clone()).collect();
    assert!(!joined.contains("truncated"));
}

#[test]
fn strict_schema_only_on_final_turn() {
    let body = serde_json::json!({
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
    let huge = "word ".repeat(45_000 * 4);
    let parsed = parse_openai_messages(&[serde_json::json!({
        "role": "user",
        "content": huge
    })]);
    let plan = plan_completion_turns(&parsed, "base", schema.as_deref(), 4_096);
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

#[test]
fn pow_cache_reuses_entry_for_same_challenge() {
    use crate::{
        api::pow_challenge::PowChallenge,
        pow::{ALGORITHM, cache::PowCache},
    };

    let cache = PowCache::new();
    let challenge = PowChallenge {
        algorithm: ALGORITHM.into(),
        challenge: "abc".into(),
        salt: "s".into(),
        signature: "sig".into(),
        difficulty: 1000,
        expire_at: 9_999_999_999,
        expire_after: 0,
        target_path: "/api/v0/chat/completion".into(),
    };
    cache.store("token123456", "sess", challenge.clone(), "pow1".into());
    assert_eq!(
        cache.get("token123456", "sess", &challenge).as_deref(),
        Some("pow1")
    );
    assert_eq!(cache.cache_hits(), 1);
}
