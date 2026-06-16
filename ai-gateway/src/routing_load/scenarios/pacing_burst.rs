use std::time::Duration;

use crate::router::pacing::{PacingGate, PacingLimits};

pub async fn run() {
    let gate = PacingGate::new(PacingLimits {
        rpm: 4,
        concurrent: 1,
        min_interval: Duration::from_secs(12),
        max_queue_wait: Duration::from_secs(30),
    });
    let first = gate.acquire().await.expect("first permit");
    let second =
        tokio::time::timeout(Duration::from_millis(50), gate.acquire()).await;
    assert!(second.is_err(), "second concurrent acquire should wait");
    drop(first);
    gate.acquire().await.expect("second permit after release");
}
