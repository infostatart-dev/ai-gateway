use serde_json::Value;

use super::model::{
    ContentPath, SearchResult, SseDelta, append_search_citations,
    format_stream_content, is_search_model, is_thinking_model,
};

#[derive(Debug, Clone, Default)]
pub struct CollectedSse {
    pub content: String,
    pub reasoning_content: String,
}

#[derive(Debug, Default)]
pub struct SseParser {
    current_path: ContentPath,
    search_results: Vec<SearchResult>,
    content: String,
    reasoning_content: String,
    model: String,
    thinking_model: bool,
}

impl SseParser {
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_string(),
            thinking_model: is_thinking_model(model),
            ..Default::default()
        }
    }

    pub fn search_results(&self) -> &[SearchResult] {
        &self.search_results
    }

    pub fn feed_line(&mut self, line: &str) -> Vec<SseDelta> {
        let payload = match line.strip_prefix("data:") {
            Some(p) => p.trim(),
            None => return Vec::new(),
        };
        if payload.is_empty() {
            return Vec::new();
        }
        if payload == "[DONE]" {
            return Vec::new();
        }
        let Ok(data) = serde_json::from_str::<Value>(payload) else {
            return Vec::new();
        };
        self.handle_event(&data)
    }

    pub fn finish_with_citations(self) -> CollectedSse {
        let citations =
            append_search_citations(&self.search_results, &self.model);
        let mut collected = CollectedSse {
            content: self.content,
            reasoning_content: self.reasoning_content,
        };
        if !citations.is_empty() {
            collected.content.push_str("\n\n");
            collected.content.push_str(&citations);
        }
        collected
    }

    fn handle_event(&mut self, data: &Value) -> Vec<SseDelta> {
        let mut deltas = Vec::new();
        let p = data.get("p").and_then(Value::as_str).unwrap_or("");
        let o = data.get("o").and_then(Value::as_str);
        let v = data.get("v");

        if let Some(obj) = v.filter(|x| x.is_object())
            && let Some(resp) = obj.get("response")
        {
            if resp.get("thinking_enabled") == Some(&Value::Bool(true)) {
                self.current_path = ContentPath::Thinking;
            } else if resp.get("thinking_enabled") == Some(&Value::Bool(false))
            {
                self.current_path = ContentPath::Content;
            }
            if let Some(frags) = resp.get("fragments").and_then(Value::as_array)
            {
                for frag in frags {
                    deltas.extend(self.handle_fragment(frag, false));
                }
            }
        }

        if p == "response/fragments" {
            match v {
                Some(Value::Array(arr)) => {
                    for frag in arr {
                        deltas.extend(self.handle_fragment(frag, true));
                    }
                }
                Some(obj) if obj.is_object() => {
                    deltas.extend(self.handle_fragment(obj, true))
                }
                _ => {}
            }
        }

        if p == "response" && v.and_then(Value::as_array).is_some() {
            for entry in v.and_then(Value::as_array).into_iter().flatten() {
                if entry.get("p") == Some(&Value::String("response".into()))
                    && entry.pointer("/v/thinking_enabled")
                        == Some(&Value::Bool(true))
                {
                    self.current_path = ContentPath::Thinking;
                }
            }
        }

        if p == "response/search_status" {
            return deltas;
        }

        if p == "response/search_results"
            && v.and_then(Value::as_array).is_some()
        {
            self.handle_search_results(v.unwrap(), o);
            return deltas;
        }

        if let Some(text) = v.and_then(Value::as_str) {
            deltas.extend(self.send_by_path(text));
        } else if p == "response"
            && let Some(arr) = v.and_then(Value::as_array)
        {
            for entry in arr {
                if let Some(inner) = entry.get("v").and_then(Value::as_array) {
                    let joined: String = inner
                        .iter()
                        .filter_map(|item| {
                            item.get("content").and_then(Value::as_str)
                        })
                        .collect();
                    if !joined.is_empty() {
                        deltas.extend(self.send_by_path(&joined));
                    }
                }
            }
        }
        deltas
    }

    fn handle_search_results(&mut self, v: &Value, o: Option<&str>) {
        let arr = v.as_array().unwrap();
        if o != Some("BATCH") {
            self.search_results.clear();
            for item in arr {
                self.search_results.push(parse_search_result(item));
            }
            return;
        }
        static RE: std::sync::LazyLock<regex::Regex> =
            std::sync::LazyLock::new(|| {
                regex::Regex::new(r"^(\d+)/cite_index$").unwrap()
            });
        for op in arr {
            let path = op.get("p").and_then(Value::as_str).unwrap_or("");
            if let Some(caps) = RE.captures(path) {
                let idx: usize = caps[1].parse().unwrap_or(0);
                if let Some(slot) = self.search_results.get_mut(idx) {
                    slot.cite_index =
                        op.get("v").and_then(Value::as_u64).map(|n| n as u32);
                }
            }
        }
    }

    fn handle_fragment(
        &mut self,
        frag: &Value,
        set_path_from_type: bool,
    ) -> Vec<SseDelta> {
        if set_path_from_type {
            self.apply_fragment_type(frag);
        }
        let content = frag.get("content").and_then(Value::as_str).unwrap_or("");
        if content.is_empty() {
            return Vec::new();
        }
        if !set_path_from_type {
            self.apply_fragment_type(frag);
        }
        self.send_by_path(content)
    }

    fn apply_fragment_type(&mut self, frag: &Value) {
        match frag
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_ascii_uppercase()
            .as_str()
        {
            "THINK" => self.current_path = ContentPath::Thinking,
            "ANSWER" | "RESPONSE" => self.current_path = ContentPath::Content,
            _ => {}
        }
    }

    fn send_by_path(&mut self, raw: &str) -> Vec<SseDelta> {
        let text = format_stream_content(raw, &self.model);
        if text.is_empty() {
            return Vec::new();
        }
        let path = match self.current_path {
            ContentPath::Unset if self.thinking_model => ContentPath::Thinking,
            ContentPath::Unset if is_search_model(&self.model) => {
                ContentPath::Content
            }
            other => other,
        };
        match path {
            ContentPath::Thinking => {
                self.reasoning_content.push_str(&text);
                vec![SseDelta::Reasoning(text)]
            }
            _ => {
                self.content.push_str(&text);
                vec![SseDelta::Content(text)]
            }
        }
    }
}

fn parse_search_result(v: &Value) -> SearchResult {
    SearchResult {
        cite_index: v
            .get("cite_index")
            .and_then(Value::as_u64)
            .map(|n| n as u32),
        title: v.get("title").and_then(Value::as_str).map(str::to_string),
        url: v.get("url").and_then(Value::as_str).map(str::to_string),
    }
}

pub fn collect_sse(raw: &str, model: &str) -> CollectedSse {
    let mut parser = SseParser::new(model);
    for line in raw.lines() {
        parser.feed_line(line);
    }
    parser.finish_with_citations()
}
