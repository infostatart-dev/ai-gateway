use std::time::Duration;

use ai_gateway::middleware::decision::{policy::Tier, shaping::TrafficShaper};

use crate::rl::support::*;

pub async fn run() {
    let shaper = TrafficShaper::new(32, 16, 16, 16, 16);
    let mut permits = Vec::new();
    for _ in 0..16 {
        permits.push(
            shaper
                .acquire(Tier::Free, Duration::from_secs(1))
                .await
                .expect("free slot"),
        );
    }
    let blocked = shaper.acquire(Tier::Free, Duration::from_millis(50)).await;
    assert!(blocked.is_err(), "free tier should be saturated");
}
