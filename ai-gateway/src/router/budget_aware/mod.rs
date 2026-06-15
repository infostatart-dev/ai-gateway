//! Budget-ranked routing with cooldown-aware ordering and failover.

mod credential_balance;
mod call;
mod cooldown;
mod dispatch;
mod factory;
mod failover_integration;
mod failover_loop;
mod health;
mod new_router;
mod rank;
mod rank_score;
mod selection;
mod selection_mode;
mod sort;
mod structured_output;
mod tower;
mod types;

#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "testing"))]
mod chatgpt_web_tests;
#[cfg(all(test, feature = "testing"))]
mod credential_failover;

pub(crate) use rank::default_provider_budget_rank;
pub use types::BudgetAwareRouter;
