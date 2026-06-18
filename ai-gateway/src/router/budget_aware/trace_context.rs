//! Terminal route observability: quota scope + model ladder position.

use crate::{
    config::{
        model_ladder::ModelLadderRegistry,
        provider_limits::{ProviderLimitCatalog, ProviderQuotaProfile},
    },
    router::budget_aware::types::BudgetCandidate,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalRouteContext {
    pub quota_scope: Option<String>,
    pub model_ladder_band: Option<String>,
    pub model_ladder_position: Option<u16>,
}

#[must_use]
pub fn terminal_route_context(
    limits: &ProviderLimitCatalog,
    candidate: &BudgetCandidate,
) -> TerminalRouteContext {
    let quota_scope = Some(
        match limits.quota_profile(&candidate.capability.provider) {
            ProviderQuotaProfile::PerModel => "model",
            ProviderQuotaProfile::PerSlot => "slot",
            ProviderQuotaProfile::PerSession => "session",
        }
        .to_string(),
    );
    let registry = ModelLadderRegistry::default();
    let ladder = registry.position(
        &candidate.capability.provider,
        &candidate.credential_tier,
        &candidate.capability.model.to_string(),
    );
    TerminalRouteContext {
        quota_scope,
        model_ladder_band: ladder.as_ref().map(|p| p.band.as_str().to_string()),
        model_ladder_position: ladder.map(|p| p.position),
    }
}

#[cfg(all(test, feature = "testing"))]
mod tests {
    use super::*;
    use crate::{
        app_state::AppState, config::model_ladder::LadderBand,
        router::budget_aware::test_support::gemini_model_candidate,
    };

    #[tokio::test]
    async fn gemini_terminal_trace_includes_ladder_fields() {
        let app_state = AppState::test_default().await;
        let candidate = gemini_model_candidate(
            &app_state,
            "gemini-free-8",
            "gemini-3-flash-preview",
        )
        .await;
        let ctx = terminal_route_context(
            &app_state.config().provider_limits,
            &candidate,
        );
        assert_eq!(ctx.quota_scope.as_deref(), Some("model"));
        assert_eq!(ctx.model_ladder_band.as_deref(), Some("fast"));
        assert_eq!(ctx.model_ladder_position, Some(0));
        let registry =
            crate::config::model_ladder::ModelLadderRegistry::default();
        assert_eq!(
            registry
                .position(
                    &candidate.capability.provider,
                    &candidate.credential_tier,
                    "gemini-3-flash-preview",
                )
                .expect("pos")
                .band,
            LadderBand::Fast
        );
    }
}
