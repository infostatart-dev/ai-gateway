//! Budget-ranked routing with cooldown-aware ordering and failover.

mod call;
mod cooldown;
mod dispatch;
mod failover_loop;
mod health;
mod new_router;
mod rank;
mod rank_score;
mod selection;
mod sort;
mod tower;
mod types;

#[cfg(test)]
mod tests;

pub use types::BudgetAwareRouter;
