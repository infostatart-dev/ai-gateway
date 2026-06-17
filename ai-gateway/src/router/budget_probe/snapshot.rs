use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub struct KeyInfoSnapshot {
    pub limit_remaining: Option<f64>,
    pub is_free_tier: bool,
    pub probed_at: Instant,
}

impl KeyInfoSnapshot {
    #[must_use]
    pub fn blocks_paid_route(&self, model: &str) -> bool {
        if is_free_model_route(model) {
            return false;
        }
        if self.is_free_tier && self.limit_remaining.is_none() {
            return false;
        }
        matches!(self.limit_remaining, Some(v) if v <= 0.0)
    }
}

#[must_use]
pub fn is_free_model_route(model: &str) -> bool {
    model.ends_with(":free") || model.contains("/free")
}
