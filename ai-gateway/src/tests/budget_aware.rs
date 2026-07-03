//! Public test facade for budget-aware routing integration tests.

pub use crate::{
    config::credentials::ProviderCredentialId,
    metrics::provider::attempt::CallOutcome,
    router::{
        budget_aware::{
            CredentialHealthRegistry, RouteBinding, WorkUnitRouteMemory,
            chatgpt_candidate, deepseek_model_candidate, empty_router,
            gemini_model_candidate, intent_autodefault_router,
            named_model_candidate, openrouter_model_candidate,
            ordered_candidates_for_source,
            plan::{
                build::MAX_PLAN_HOPS, plan_route_chain, score::hash_bias,
                snapshot::QuotaSnapshot,
            },
        },
        capability::RequestRequirements,
        pacing::{PacingRegistry, gate::PacingGate, limits::PacingLimits},
        quota_admission::{
            AdmissionVerdict, BlockedReason, PacingAdmissionScope,
            evaluate_candidate, evaluate_pacing_admission,
        },
    },
    types::provider::InferenceProvider,
};
