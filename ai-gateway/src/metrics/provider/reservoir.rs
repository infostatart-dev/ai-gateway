use std::cmp::Ordering;

/// Fixed-size reservoir for approximate latency percentiles (ops dashboards).
#[derive(Debug, Default)]
pub struct LatencyReservoir {
    samples: Vec<f64>,
    cap: usize,
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
impl LatencyReservoir {
    #[must_use]
    pub const fn new(cap: usize) -> Self {
        Self {
            samples: Vec::new(),
            cap,
        }
    }

    pub fn record(&mut self, value: f64) {
        if !value.is_finite() || value < 0.0 {
            return;
        }
        if self.samples.len() < self.cap {
            self.samples.push(value);
            return;
        }
        let idx = self.samples.len() % self.cap;
        if idx < self.samples.len() {
            self.samples[idx] = value;
        }
    }

    #[must_use]
    pub fn percentile(&self, p: f64) -> Option<f64> {
        if self.samples.is_empty() {
            return None;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        let idx = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
        let idx = idx.saturating_sub(1).min(sorted.len() - 1);
        Some(sorted[idx])
    }

    #[must_use]
    pub fn average(&self) -> Option<f64> {
        if self.samples.is_empty() {
            return None;
        }
        Some(self.samples.iter().sum::<f64>() / self.samples.len() as f64)
    }
}
