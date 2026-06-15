use std::{collections::HashMap, sync::LazyLock};

static MODEL_MAP: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        HashMap::from([
            ("gpt-5.3-instant", "gpt-5-3-instant"),
            ("gpt-5.3", "gpt-5-3"),
            ("gpt-5.3-mini", "gpt-5-3-mini"),
            ("gpt-5.5-instant", "gpt-5-5-instant"),
            ("gpt-5.5-thinking", "gpt-5-5-thinking"),
            ("gpt-5.4-thinking", "gpt-5-4-thinking"),
            ("gpt-5.4-thinking-mini", "gpt-5-4-t-mini"),
            ("gpt-5.2-instant", "gpt-5-2-instant"),
            ("gpt-5.2", "gpt-5-2"),
            ("gpt-5.2-thinking", "gpt-5-2-thinking"),
            ("gpt-5.1", "gpt-5-1"),
            ("gpt-5", "gpt-5"),
            ("gpt-5-mini", "gpt-5-mini"),
            ("o3", "o3"),
        ])
    });

pub fn map_model(model: &str) -> String {
    MODEL_MAP
        .get(model)
        .map(|s| (*s).to_string())
        .unwrap_or_else(|| model.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_dot_to_dash() {
        assert_eq!(map_model("gpt-5-mini"), "gpt-5-mini");
        assert_eq!(map_model("gpt-5.3-instant"), "gpt-5-3-instant");
    }
}
