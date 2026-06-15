/// Conservative chars-per-token estimate for Latin/Cyrillic mixed text.
pub const CHARS_PER_TOKEN: usize = 3;

pub const TRUNCATION_PREFIX: &str =
    "[... earlier content truncated to fit provider context limit ...]\n\n";

#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
    let chars = text.chars().count();
    chars.div_ceil(CHARS_PER_TOKEN)
}

/// Keep the tail of `text` within `max_tokens` (dossiers: recent facts at end).
#[must_use]
pub fn trim_tail_tokens(text: &str, max_tokens: usize) -> String {
    if estimate_tokens(text) <= max_tokens {
        return text.to_string();
    }
    let max_chars = max_tokens.saturating_mul(CHARS_PER_TOKEN);
    let tail: String = text.chars().rev().take(max_chars).collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{TRUNCATION_PREFIX}{tail}")
}
