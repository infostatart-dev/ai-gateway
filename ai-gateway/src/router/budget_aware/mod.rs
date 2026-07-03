//! Budget-ranked routing with cooldown-aware ordering and failover.

mod admission;
mod call;
mod cooldown;
mod credential_balance;
mod dispatch;
mod factory;
mod failover_integration;
mod failover_loop;
mod failure;
mod health;
mod health_registry;
mod intent_acceptance;
mod intent_selection;
mod ladder_filter;
mod ladder_rank;
mod memory;
mod new_router;
mod payload;
pub(crate) mod plan;
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
mod trace_context;
mod types;

pub use trace::{ChatGptWebTrace, DeepSeekWebTrace};

#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "testing"))]
mod chatgpt_web_tests;
#[cfg(all(test, feature = "testing"))]
mod credential_failover;

#[cfg(feature = "testing")]
pub use call::{
    clear_test_call_responses, install_upstream_mock, push_test_call_response,
    push_test_call_response_for_credential,
};
#[cfg(feature = "testing")]
pub use failover_loop::run_failover_candidates;
pub use health_registry::{CredentialHealthRegistry, RoutingHealthSnapshot};
#[cfg(feature = "testing")]
pub use memory::RouteBinding;
pub use memory::WorkUnitRouteMemory;
pub(crate) use rank::default_provider_budget_rank;
#[cfg(feature = "testing")]
pub use test_support::{
    balance_ranked, chatgpt_candidate, deep_paid_candidate,
    deepseek_model_candidate, deepseek_slots, empty_router, gemini_candidate,
    gemini_model_candidate, gemini_slots, groq_candidate,
    intent_autodefault_router, named_model_candidate,
    openrouter_model_candidate, ordered_candidates,
    ordered_candidates_for_source, request_parts, router_app_state,
    router_with_candidates, scout_candidate,
};
pub use types::{BudgetAwareRouter, BudgetCandidate};
