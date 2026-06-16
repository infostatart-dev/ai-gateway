use std::sync::LazyLock;

use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentPath {
    #[default]
    Unset,
    Thinking,
    Content,
}

#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    pub cite_index: Option<u32>,
    pub title: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SseDelta {
    Content(String),
    Reasoning(String),
}

pub fn is_thinking_model(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.contains("think") || m.contains("r1") || m.contains("reason")
}

pub fn is_search_model(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.contains("search") || m.contains("fold")
}

pub fn format_stream_content(raw: &str, model: &str) -> String {
    let mut text = raw.replace("FINISHED", "");
    static LEADING: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(SEARCH|WEB_SEARCH|SEARCHING)\s*").expect("regex")
    });
    text = LEADING.replace_all(&text, "").to_string();
    if !is_search_model(model) {
        return text;
    }
    static CITE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\[citation:(\d+)\]").expect("regex"));
    if model.to_ascii_lowercase().contains("search-silent") {
        return CITE.replace_all(&text, "").to_string();
    }
    CITE.replace_all(&text, "[$1]").to_string()
}

pub fn append_search_citations(
    results: &[SearchResult],
    model: &str,
) -> String {
    if results.is_empty()
        || model.to_ascii_lowercase().contains("search-silent")
    {
        return String::new();
    }
    let mut indexed: Vec<_> = results
        .iter()
        .filter_map(|r| r.cite_index.map(|i| (i, r)))
        .collect();
    indexed.sort_by_key(|(i, _)| *i);
    indexed
        .into_iter()
        .filter_map(|(_, r)| {
            let title = r.title.as_deref()?;
            let url = r.url.as_deref()?;
            r.cite_index.map(|i| format!("[{i}]: [{title}]({url})"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}
