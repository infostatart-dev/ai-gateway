use std::time::Duration;

use ai_gateway::tests::routing::{PacingGate, PacingLimits};

use crate::rl::support::*;

pub async fn run() {
    let gate = PacingGate::new(PacingLimits {
        concurrent: 4,
        rpm: u32::MAX,
        tpm: None,
        rpd: Some(1),
        tpd: None,
        daily_reset_utc_hour: 0,
        min_interval: Duration::ZERO,
        max_queue_wait: Duration::from_secs(1),
    });
    gate.acquire(0).await.expect("first daily request");
    assert!(
        gate.acquire(0).await.is_err(),
        "second request should be rejected after rpd exhaustion"
    );
}
