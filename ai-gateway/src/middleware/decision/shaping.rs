use std::{sync::Arc, time::Duration};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{
    config::decision::TierCascade, middleware::decision::policy::Tier,
};

pub struct TrafficShaper {
    global_limit: Arc<Semaphore>,
    free_tier_limit: Arc<Semaphore>,
    freemium_tier_limit: Arc<Semaphore>,
    paid_tier_limit: Arc<Semaphore>,
    provider_limit: Arc<Semaphore>,
}

impl TrafficShaper {
    #[must_use]
    pub fn new(
        global: usize,
        free_tier: usize,
        freemium_tier: usize,
        paid_tier: usize,
        provider: usize,
    ) -> Self {
        Self {
            global_limit: Arc::new(Semaphore::new(global)),
            free_tier_limit: Arc::new(Semaphore::new(free_tier)),
            freemium_tier_limit: Arc::new(Semaphore::new(freemium_tier)),
            paid_tier_limit: Arc::new(Semaphore::new(paid_tier)),
            provider_limit: Arc::new(Semaphore::new(provider)),
        }
    }

    /// Acquire combined permit for a single tier (no cascade).
    pub async fn acquire(
        &self,
        tier: Tier,
        timeout: Duration,
    ) -> Result<CombinedPermit, String> {
        tokio::time::timeout(timeout, self.acquire_inner(tier))
            .await
            .map_err(|_| "timeout acquiring traffic slot".to_string())?
    }

    /// Acquire with tier cascade: on start-tier exhaustion, try the next tier
    /// in cascade order. Each attempt uses the same `timeout`; returns the
    /// first success.
    pub async fn acquire_with_cascade(
        &self,
        start_tier: Tier,
        cascade: TierCascade,
        timeout: Duration,
    ) -> Result<AcquireOutcome, String> {
        let chain = cascade_chain(start_tier, cascade);
        let mut last_error: Option<String> = None;
        for tier in chain {
            match tokio::time::timeout(timeout, self.acquire_inner(tier)).await
            {
                Ok(Ok(permit)) => {
                    return Ok(AcquireOutcome { tier, permit });
                }
                Ok(Err(e)) => last_error = Some(e),
                Err(_) => {
                    last_error =
                        Some(format!("timeout acquiring {tier:?} slot"));
                }
            }
        }
        Err(last_error
            .unwrap_or_else(|| "no tiers attempted in cascade".to_string()))
    }

    async fn acquire_inner(
        &self,
        tier: Tier,
    ) -> Result<CombinedPermit, String> {
        let global = self
            .global_limit
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "global shaper closed".to_string())?;
        let tier_permit = match tier {
            Tier::Free => self.free_tier_limit.clone().acquire_owned().await,
            Tier::Freemium => {
                self.freemium_tier_limit.clone().acquire_owned().await
            }
            Tier::Paid => self.paid_tier_limit.clone().acquire_owned().await,
        }
        .map_err(|_| "tier shaper closed".to_string())?;
        let provider = self
            .provider_limit
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "provider shaper closed".to_string())?;

        Ok(CombinedPermit {
            _global: global,
            _tier: tier_permit,
            _provider: provider,
        })
    }
}

pub struct CombinedPermit {
    _global: OwnedSemaphorePermit,
    _tier: OwnedSemaphorePermit,
    _provider: OwnedSemaphorePermit,
}

/// Cascade acquire result: winning tier plus permit.
pub struct AcquireOutcome {
    pub tier: Tier,
    pub permit: CombinedPermit,
}

/// Ordered tier list from `start` per cascade mode. `OnlyTier` yields a single
/// element.
fn cascade_chain(start: Tier, cascade: TierCascade) -> Vec<Tier> {
    match cascade {
        TierCascade::OnlyTier => vec![start],
        TierCascade::PaidDown => {
            // paid → freemium → free, sliced from `start`.
            let order = [Tier::Paid, Tier::Freemium, Tier::Free];
            slice_from(start, &order)
        }
        TierCascade::FreeUp => {
            let order = [Tier::Free, Tier::Freemium, Tier::Paid];
            slice_from(start, &order)
        }
    }
}

fn slice_from(start: Tier, order: &[Tier]) -> Vec<Tier> {
    if let Some(idx) = order.iter().position(|t| *t == start) {
        order[idx..].to_vec()
    } else {
        vec![start]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_tier_returns_single_start() {
        assert_eq!(
            cascade_chain(Tier::Paid, TierCascade::OnlyTier),
            vec![Tier::Paid]
        );
        assert_eq!(
            cascade_chain(Tier::Freemium, TierCascade::OnlyTier),
            vec![Tier::Freemium]
        );
        assert_eq!(
            cascade_chain(Tier::Free, TierCascade::OnlyTier),
            vec![Tier::Free]
        );
    }

    #[test]
    fn paid_down_chain_starts_from_given_tier() {
        assert_eq!(
            cascade_chain(Tier::Paid, TierCascade::PaidDown),
            vec![Tier::Paid, Tier::Freemium, Tier::Free]
        );
        assert_eq!(
            cascade_chain(Tier::Freemium, TierCascade::PaidDown),
            vec![Tier::Freemium, Tier::Free]
        );
        assert_eq!(
            cascade_chain(Tier::Free, TierCascade::PaidDown),
            vec![Tier::Free]
        );
    }

    #[test]
    fn free_up_chain_starts_from_given_tier() {
        assert_eq!(
            cascade_chain(Tier::Free, TierCascade::FreeUp),
            vec![Tier::Free, Tier::Freemium, Tier::Paid]
        );
        assert_eq!(
            cascade_chain(Tier::Freemium, TierCascade::FreeUp),
            vec![Tier::Freemium, Tier::Paid]
        );
        assert_eq!(
            cascade_chain(Tier::Paid, TierCascade::FreeUp),
            vec![Tier::Paid]
        );
    }

    #[tokio::test]
    async fn acquire_succeeds_when_slot_free() {
        let shaper = TrafficShaper::new(10, 5, 5, 5, 10);
        let permit = shaper
            .acquire(Tier::Freemium, Duration::from_millis(100))
            .await
            .expect("permit");
        drop(permit);
    }

    #[tokio::test]
    async fn acquire_with_cascade_falls_back_to_next_tier() {
        // Free tier saturated (limit=1, permit held); freemium has capacity.
        let shaper = TrafficShaper::new(10, 1, 5, 5, 10);
        let _hold = shaper
            .acquire(Tier::Free, Duration::from_millis(50))
            .await
            .expect("hold free");

        let outcome = shaper
            .acquire_with_cascade(
                Tier::Free,
                TierCascade::FreeUp,
                Duration::from_millis(50),
            )
            .await
            .expect("outcome");

        assert_eq!(outcome.tier, Tier::Freemium);
    }

    #[tokio::test]
    async fn acquire_with_only_tier_does_not_cascade() {
        let shaper = TrafficShaper::new(10, 1, 5, 5, 10);
        let _hold = shaper
            .acquire(Tier::Free, Duration::from_millis(50))
            .await
            .expect("hold free");

        let result = shaper
            .acquire_with_cascade(
                Tier::Free,
                TierCascade::OnlyTier,
                Duration::from_millis(50),
            )
            .await;

        assert!(result.is_err());
    }
}
