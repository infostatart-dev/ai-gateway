use std::sync::OnceLock;

use tiktoken_rs::{CoreBPE, o200k_base};

/// Shared `o200k_base` BPE (GPT-4o/GPT-5 family). Used as the canonical
/// estimator across OpenAI-compatible providers; per-provider divergence is
/// absorbed by the routing safety margin.
fn bpe() -> &'static CoreBPE {
    static BPE: OnceLock<CoreBPE> = OnceLock::new();
    BPE.get_or_init(|| o200k_base().expect("embedded o200k_base ranks"))
}

/// Count BPE tokens in `text`, saturating at `u32::MAX`.
#[must_use]
pub fn count_tokens(text: &str) -> u32 {
    let count = bpe().encode_ordinary(text).len();
    u32::try_from(count).unwrap_or(u32::MAX)
}
