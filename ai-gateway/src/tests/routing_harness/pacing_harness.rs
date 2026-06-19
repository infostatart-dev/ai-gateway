use crate::{
    app_state::AppState,
    config::{
        catalog_limit_resolve::normalize_model_slug,
        credentials::ProviderCredentialId,
    },
    types::provider::InferenceProvider,
};

pub async fn saturate_model_pacing(
    app_state: &AppState,
    credential: &str,
    model: &str,
    count: u32,
) {
    let slug = normalize_model_slug(model);
    let gate = app_state
        .upstream_pacing()
        .gate_for(
            &InferenceProvider::GoogleGemini,
            Some(&ProviderCredentialId::new(credential)),
            Some("free"),
            Some(slug.as_str()),
        )
        .expect("pacing gate");
    for _ in 0..count {
        let permit = gate.acquire(0).await.expect("pacing acquire");
        drop(permit);
    }
}
