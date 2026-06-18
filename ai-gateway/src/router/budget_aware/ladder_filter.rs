use crate::{
    config::{
        catalog_limit_resolve::normalize_model_slug,
        model_ladder::ModelLadderRegistry,
        provider_limits::{ProviderLimitCatalog, ProviderQuotaProfile},
    },
    router::budget_aware::types::BudgetCandidate,
};

#[must_use]
pub fn ladder_eligible(
    candidate: &BudgetCandidate,
    limits: &ProviderLimitCatalog,
    ladders: &ModelLadderRegistry,
) -> bool {
    if limits.quota_profile(&candidate.capability.provider)
        != ProviderQuotaProfile::PerModel
    {
        return true;
    }
    let ladder_slugs = ladders.ladder_model_slugs(
        &candidate.capability.provider,
        &candidate.credential_tier,
    );
    if ladder_slugs.is_empty() {
        return true;
    }
    let slug = normalize_model_slug(&candidate.capability.model.to_string());
    ladder_slugs.iter().any(|entry| entry == &slug)
}

pub fn retain_ladder_eligible(
    candidates: &mut Vec<BudgetCandidate>,
    limits: &ProviderLimitCatalog,
    ladders: &ModelLadderRegistry,
) {
    candidates.retain(|candidate| ladder_eligible(candidate, limits, ladders));
}

#[cfg(all(test, feature = "testing"))]
mod tests {
    use super::*;
    use crate::{
        app_state::AppState,
        config::model_ladder::ModelLadderRegistry,
        router::budget_aware::test_support::{
            gemini_candidate, gemini_model_candidate,
        },
        types::provider::InferenceProvider,
    };

    #[tokio::test]
    async fn openrouter_free_keeps_only_ladder_slugs() {
        use crate::router::budget_aware::test_support::openrouter_model_candidate;

        let app_state = AppState::test_default().await;
        let limits = &app_state.config().provider_limits;
        let ladders = ModelLadderRegistry::default();
        let mut candidates = vec![
            openrouter_model_candidate(
                &app_state,
                "openrouter-default",
                "openai/gpt-oss-120b:free",
            )
            .await,
            openrouter_model_candidate(
                &app_state,
                "openrouter-default",
                "openai/gpt-4o-mini",
            )
            .await,
            openrouter_model_candidate(
                &app_state,
                "openrouter-default",
                "nvidia/nemotron-3-nano-30b-a3b:free",
            )
            .await,
        ];
        retain_ladder_eligible(&mut candidates, limits, &ladders);
        let slugs: Vec<_> = candidates
            .iter()
            .map(|c| c.capability.model.to_string())
            .collect();
        assert!(slugs.contains(&"openai/gpt-oss-120b:free".to_string()));
        assert!(
            slugs.contains(&"nvidia/nemotron-3-nano-30b-a3b:free".to_string())
        );
        assert!(!slugs.contains(&"openai/gpt-4o-mini".to_string()));
    }

    #[tokio::test]
    async fn free_gemini_keeps_only_ladder_slugs() {
        let app_state = AppState::test_default().await;
        let limits = &app_state.config().provider_limits;
        let ladders = ModelLadderRegistry::default();
        let mut candidates = vec![
            gemini_model_candidate(
                &app_state,
                "gemini-free-8",
                "gemini-3-flash-preview",
            )
            .await,
            gemini_model_candidate(
                &app_state,
                "gemini-free-8",
                "gemini-2.5-pro",
            )
            .await,
            gemini_model_candidate(
                &app_state,
                "gemini-free-8",
                "gemini-3.1-flash-lite",
            )
            .await,
        ];
        retain_ladder_eligible(&mut candidates, limits, &ladders);
        let slugs: Vec<_> = candidates
            .iter()
            .map(|c| c.capability.model.to_string())
            .collect();
        assert!(slugs.contains(&"gemini-3-flash-preview".to_string()));
        assert!(slugs.contains(&"gemini-3.1-flash-lite".to_string()));
        assert!(!slugs.contains(&"gemini-2.5-pro".to_string()));
    }

    #[tokio::test]
    async fn paid_gemini_default_unfiltered_without_ladder_tier() {
        let app_state = AppState::test_default().await;
        let limits = &app_state.config().provider_limits;
        let ladders = ModelLadderRegistry::default();
        let candidate =
            gemini_candidate(&app_state, "gemini-default", 10, "paid-key")
                .await;
        assert_eq!(
            candidate.capability.provider,
            InferenceProvider::GoogleGemini
        );
        assert_eq!(candidate.credential_tier, "tier-3");
        assert!(ladder_eligible(&candidate, limits, &ladders));
    }
}
