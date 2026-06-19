use std::collections::HashMap;

use crate::{
    router::routed_identity::REAL_MODE_MODEL_AND_PROVIDER,
    types::response::Response,
};

#[must_use]
pub fn routed_identity(response: &Response) -> String {
    response
        .headers()
        .get(REAL_MODE_MODEL_AND_PROVIDER)
        .expect("routed identity header")
        .to_str()
        .unwrap()
        .to_string()
}

#[must_use]
pub fn terminal_provider_counts(
    identities: &[String],
) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for identity in identities {
        let credential = identity
            .split('/')
            .next()
            .unwrap_or(identity.as_str())
            .to_string();
        *counts.entry(credential).or_insert(0) += 1;
    }
    counts
}

#[allow(clippy::implicit_hasher)]
pub fn assert_fairness_band(
    counts: &HashMap<String, usize>,
    credentials: &[&str],
    total: usize,
    tolerance_percent: u32,
) {
    let n = credentials.len();
    assert!(n > 0, "fairness band requires at least one credential");
    let tp = tolerance_percent as usize;
    let low = total.saturating_mul(100 - tp) / (100 * n);
    let high =
        total.saturating_mul(100 + tp).saturating_add(100 * n - 1) / (100 * n);
    for credential in credentials {
        let got = counts.get(*credential).copied().unwrap_or(0);
        assert!(
            (low..=high).contains(&got),
            "credential {credential} got {got}, expected {low}..={high}"
        );
    }
}

#[allow(clippy::implicit_hasher)]
pub fn assert_zero_terminal_credentials(
    counts: &HashMap<String, usize>,
    credentials: &[&str],
) {
    for credential in credentials {
        assert_eq!(
            counts.get(*credential).copied().unwrap_or(0),
            0,
            "expected zero terminals for {credential}"
        );
    }
}
