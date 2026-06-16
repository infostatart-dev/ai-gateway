mod collect;
mod model;
mod stream;

pub use collect::{CollectedSse, SseParser, collect_sse};
pub use model::{SearchResult, SseDelta};
pub use stream::{build_non_stream_response, transform_sse_to_openai};

#[cfg(test)]
mod tests;
