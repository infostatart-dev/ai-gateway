//! Emulated autodefault: rewrite API-key provider `base-url` → local
//! upstream-emulator.

mod upstream;

pub use upstream::{
    apply_upstream_binding, emulated_enabled, emulator_base_url,
};

/// When `AI_GATEWAY_EMULATED=1`, point every API-key upstream at the catalog
/// emulator.
pub fn apply_if_enabled(config: &mut crate::config::Config) {
    if !emulated_enabled() {
        return;
    }
    apply_upstream_binding(config);
    tracing::info!(
        base = %emulator_base_url(),
        "emulated mode: API upstream base-url rewrite enabled"
    );
}
