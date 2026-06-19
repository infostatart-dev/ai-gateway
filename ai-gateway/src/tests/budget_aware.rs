//! Public test facade for budget-aware routing integration tests.

pub use crate::{
    config::credentials::ProviderCredentialId,
    metrics::provider::attempt::CallOutcome,
    router::{
        budget_aware::{
            CredentialHealthRegistry, RouteBinding, WorkUnitRouteMemory,
            empty_router, gemini_model_candidate, openrouter_model_candidate,
            plan::{
                build::MAX_PLAN_HOPS, plan_route_chain, score::hash_bias,
                snapshot::QuotaSnapshot,
            },
        },
        capability::RequestRequirements,
        pacing::{PacingRegistry, gate::PacingGate, limits::PacingLimits},
    },
    types::provider::InferenceProvider,
};
