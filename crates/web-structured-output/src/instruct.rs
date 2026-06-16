use super::parse::JsonSchemaSpec;

const ONLY_JSON_TAIL: &str = "Output ONLY the JSON object in the message \
                              content. No markdown fences, no prose.";

const STRICT_MANDATORY: &str =
    "MANDATORY strict mode: you MUST NOT omit any required field, use wrong \
     types, or add extra properties. The response must be a single JSON \
     object that validates against the schema below — no exceptions.";

#[must_use]
pub fn build_json_object_instruction() -> String {
    format!("You must respond with a valid JSON object.\n{ONLY_JSON_TAIL}")
}

#[must_use]
pub fn build_schema_instruction(spec: &JsonSchemaSpec) -> String {
    let mut lines = Vec::new();
    if spec.strict {
        lines.push(STRICT_MANDATORY.into());
    }
    let schema = serde_json::to_string_pretty(&spec.schema)
        .unwrap_or_else(|_| "{}".into());
    lines.push(format!(
        "You must respond with valid JSON that strictly follows this JSON \
         schema:\n{schema}"
    ));
    lines.push(ONLY_JSON_TAIL.into());
    lines.join("\n")
}

/// System text without the schema block (for multi-turn context uploads).
#[must_use]
pub fn base_system_without_schema(
    system_msg: &str,
    schema_instruction: Option<&str>,
) -> String {
    let Some(schema) = schema_instruction else {
        return system_msg.trim().to_string();
    };
    system_msg
        .replace(schema, "")
        .trim()
        .trim_end_matches('\n')
        .to_string()
}
