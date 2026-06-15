//! Shared message token budgeting for web-session providers (ChatGPT Web,
//! Perplexity).

mod chunk;
mod fit;
mod parse;
mod token;
mod types;

pub use chunk::{
    final_user_suffix, fits_single_turn, plan_web_chunks, upload_part_header,
};
pub use fit::fit_parsed;
pub use parse::parse_openai_messages;
pub use token::{
    CHARS_PER_TOKEN, TRUNCATION_PREFIX, estimate_tokens, trim_tail_tokens,
};
pub use types::{
    ChunkPlan, FitReport, MessageBudget, ParsedChat, WebTurn, WebTurnKind,
};

pub const CHATGPT_WEB_CONTEXT_TOKENS: u32 = 131_072;
pub const PERPLEXITY_MAX_QUERY_CHARS: usize = 96_000;
