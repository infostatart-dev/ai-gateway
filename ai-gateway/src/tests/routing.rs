//! Public test facade for `routing_load` integration tests (`feature =
//! "testing"`).

pub use crate::{
    app_state::AppState,
    config::credentials::ProviderCredentialId,
    error::api::ApiError,
    metrics::provider::attempt::CallOutcome,
    router::{
        budget_aware::{
            BudgetAwareRouter, BudgetCandidate, CredentialHealthRegistry,
            RouteBinding, WorkUnitRouteMemory, balance_ranked,
            chatgpt_candidate, clear_test_call_responses, deep_paid_candidate,
            deepseek_model_candidate, deepseek_slots, empty_router,
            gemini_candidate, gemini_model_candidate, gemini_slots,
            groq_candidate, install_upstream_mock, intent_autodefault_router,
            openrouter_model_candidate, ordered_candidates,
            ordered_candidates_for_source, plan::plan_route_chain,
            push_test_call_response, push_test_call_response_for_credential,
            request_parts, router_app_state, router_with_candidates,
            run_failover_candidates, scout_candidate,
        },
        capability::{
            RequestRequirements, apply_payload_estimate,
            extract_requirements_from_value, extract_source_model_from_value,
        },
        intent::{
            IntentTier, RoutingIntent, extract_routing_intent,
            extract_routing_intent_from_name,
        },
        pacing::{PacingRegistry, gate::PacingGate, limits::PacingLimits},
        routed_identity::REAL_MODE_MODEL_AND_PROVIDER,
        token_estimate::{PayloadBudgetConfig, estimate_from_value},
    },
    types::{
        extensions::{CallerRequestContext, RoutePlanContext},
        provider::InferenceProvider,
        response::Response,
    },
};
