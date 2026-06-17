//! Budget-ranked routing with cooldown-aware ordering and failover.

mod call;
mod cooldown;
mod credential_balance;
mod dispatch;
mod factory;
mod failover_integration;
mod failover_loop;
mod failure;
mod health;
mod new_router;
mod payload;
mod rank;
mod rank_score;
mod selection;
mod selection_mode;
mod sort;
mod structured_output;
#[cfg(feature = "testing")]
mod test_support;
mod tower;
mod trace;
mod types;

pub use trace::{ChatGptWebTrace, DeepSeekWebTrace};

#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "testing"))]
mod chatgpt_web_tests;
#[cfg(all(test, feature = "testing"))]
mod credential_failover;

#[cfg(feature = "testing")]
pub(crate) use call::{
    clear_test_call_responses, push_test_call_response,
    push_test_call_response_for_credential,
};
#[cfg(feature = "testing")]
pub(crate) use failover_loop::run_failover_candidates;
pub(crate) use rank::default_provider_budget_rank;
#[cfg(feature = "testing")]
pub(crate) use test_support::{
    balance_ranked, chatgpt_candidate, empty_router, gemini_candidate,
    gemini_slots, groq_candidate, ordered_candidates, request_parts,
    router_with_candidates,
};
pub use types::BudgetAwareRouter;
