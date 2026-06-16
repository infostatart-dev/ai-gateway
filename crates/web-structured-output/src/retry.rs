use super::parse::StructuredOutputIssue;

pub const JSON_RETRY_SUFFIX: &str =
    "\n\nCRITICAL: Your previous response was not valid JSON. Reply with ONLY \
     a JSON object matching the schema. No markdown fences, no prose.";

pub const SCHEMA_RETRY_SUFFIX: &str =
    "\n\nCRITICAL: Your previous JSON did not match the required schema. \
     Focus carefully: output ONLY a corrected JSON object that satisfies \
     every required field and type in the schema. Preserve all factual \
     content — do not drop information. No prose, no markdown fences.";

#[must_use]
pub fn retry_suffix_for(issue: Option<StructuredOutputIssue>) -> &'static str {
    match issue {
        Some(StructuredOutputIssue::SchemaMismatch) => SCHEMA_RETRY_SUFFIX,
        Some(StructuredOutputIssue::InvalidJson) | None => JSON_RETRY_SUFFIX,
    }
}
