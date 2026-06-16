use super::{collect_sse, stream::transform_sse_to_openai};

#[test]
fn think_then_answer_fragments() {
    let raw = r#"data: {"p":"response/fragments","v":[{"type":"THINK","content":"Let me think"}]}
data: {"p":"response/fragments","v":[{"type":"ANSWER","content":"Hello world"}]}
data: [DONE]
"#;
    let collected = collect_sse(raw, "deepseek-reasoner");
    assert_eq!(collected.reasoning_content, "Let me think");
    assert_eq!(collected.content, "Hello world");

    let stream =
        String::from_utf8(transform_sse_to_openai(raw, "deepseek-reasoner"))
            .unwrap();
    assert!(stream.contains(r#""reasoning_content":"Let me think""#));
    assert!(stream.contains(r#""content":"Hello world""#));
    assert!(stream.contains("data: [DONE]"));
}

#[test]
fn search_citations_appended() {
    let raw = r#"data: {"p":"response/fragments","v":[{"type":"ANSWER","content":"Result [citation:1]"}]}
data: {"p":"response/search_results","v":[{"title":"Example","url":"https://ex.com"}]}
data: {"p":"response/search_results","o":"BATCH","v":[{"p":"0/cite_index","v":1}]}
data: [DONE]
"#;
    let collected = collect_sse(raw, "deepseek-search");
    assert!(collected.content.contains("Result [1]"));
    assert!(collected.content.contains("[1]: [Example](https://ex.com)"));
}

#[test]
fn done_marker_finishes_stream() {
    let raw = "data: {\"p\":\"response/fragments\",\"v\":[{\"type\":\"ANSWER\"\
               ,\"content\":\"Hi\"}]}\ndata: [DONE]\n";
    let collected = collect_sse(raw, "deepseek-chat");
    assert_eq!(collected.content, "Hi");
    let stream =
        String::from_utf8(transform_sse_to_openai(raw, "deepseek-chat"))
            .unwrap();
    assert!(stream.ends_with("data: [DONE]\n\n"));
}

#[test]
fn ignores_finished_status_without_closing() {
    let raw = r#"data: {"p":"response/status","v":"FINISHED"}
data: {"p":"response/search_results","v":[{"title":"Late","url":"https://late.io","cite_index":1}]}
data: {"p":"response/fragments","v":[{"type":"ANSWER","content":"Done"}]}
data: [DONE]
"#;
    let collected = collect_sse(raw, "deepseek-search");
    assert!(collected.content.contains("Done"));
    assert!(collected.content.contains("[1]: [Late](https://late.io)"));
}
