use std::cmp::Ordering;

use crate::{
    config::model_ladder::ModelLadderRegistry,
    router::budget_aware::types::BudgetCandidate,
};

#[must_use]
pub fn ladder_cmp(
    ladders: &ModelLadderRegistry,
    left: &BudgetCandidate,
    right: &BudgetCandidate,
) -> std::cmp::Ordering {
    if left.credential_id != right.credential_id {
        return left
            .credential_id
            .as_str()
            .cmp(right.credential_id.as_str());
    }
    let left_pos = ladders.position(
        &left.capability.provider,
        &left.credential_tier,
        &left.capability.model.to_string(),
    );
    let right_pos = ladders.position(
        &right.capability.provider,
        &right.credential_tier,
        &right.capability.model.to_string(),
    );
    match (left_pos, right_pos) {
        (Some(left_pos), Some(right_pos)) => left_pos
            .band_index
            .cmp(&right_pos.band_index)
            .then_with(|| left_pos.position.cmp(&right_pos.position)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
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
    async fn fast_band_ranks_before_stability_on_same_slot() {
        let app_state = AppState::test_default().await;
        let flash = gemini_model_candidate(
            &app_state,
            "gemini-free-8",
            "gemini-3-flash-preview",
        )
        .await;
        let lite = gemini_model_candidate(
            &app_state,
            "gemini-free-8",
            "gemini-2.5-flash-lite",
        )
        .await;
        let ladders = ModelLadderRegistry::default();
        assert_eq!(ladder_cmp(&ladders, &flash, &lite), Ordering::Less);
        assert_eq!(ladder_cmp(&ladders, &lite, &flash), Ordering::Greater);
    }

    #[tokio::test]
    async fn stability_not_first_when_fast_band_eligible() {
        let app_state = AppState::test_default().await;
        let flash = gemini_model_candidate(
            &app_state,
            "gemini-free-8",
            "gemini-3-flash-preview",
        )
        .await;
        let lite = gemini_model_candidate(
            &app_state,
            "gemini-free-8",
            "gemini-3.1-flash-lite",
        )
        .await;
        let stability = gemini_model_candidate(
            &app_state,
            "gemini-free-8",
            "gemini-2.5-flash-lite",
        )
        .await;
        let ladders = ModelLadderRegistry::default();
        let mut ranked = vec![stability.clone(), lite.clone(), flash.clone()];
        ranked.sort_by(|left, right| ladder_cmp(&ladders, left, right));
        assert_eq!(
            ranked[0].capability.model.to_string(),
            "gemini-3-flash-preview"
        );
        assert_eq!(
            ladders
                .position(
                    &flash.capability.provider,
                    &flash.credential_tier,
                    "gemini-2.5-flash-lite",
                )
                .expect("stability")
                .band,
            LadderBand::Capacity
        );
    }
}
