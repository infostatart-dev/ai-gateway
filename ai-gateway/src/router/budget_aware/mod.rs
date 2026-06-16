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
mod tower;
mod trace;
mod types;

#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "testing"))]
mod chatgpt_web_tests;
#[cfg(all(test, feature = "testing"))]
mod credential_failover;

pub(crate) use rank::default_provider_budget_rank;
pub use trace::DeepSeekWebTrace;
pub use types::BudgetAwareRouter;
