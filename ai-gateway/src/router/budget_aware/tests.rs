use std::time::Duration;

use super::rank::{default_provider_budget_rank, effective_budget_rank};
use crate::{
    config::cost_class::CostClass,
    router::provider_attempt::is_failoverable_status,
    types::provider::InferenceProvider,
};

fn cost_class_rank(
    cost_class: CostClass,
    budget_rank: u16,
    provider: u16,
) -> u16 {
    cost_class
        .rank_base()
        .saturating_add(budget_rank.saturating_mul(10))
        .saturating_add(provider)
}

#[test]
fn payload_too_large_triggers_provider_failover() {
    assert!(is_failoverable_status(http::StatusCode::PAYLOAD_TOO_LARGE));
}

#[test]
fn default_provider_budget_order_matches_autodefault_policy() {
    let opencode = InferenceProvider::Named("opencode".into());
    let longcat = InferenceProvider::Named("longcat".into());
    let github = InferenceProvider::Named("github-models".into());
    let mistral = InferenceProvider::Named("mistral".into());
    let bazaarlink = InferenceProvider::Named("bazaarlink".into());
    let groq = InferenceProvider::Named("groq".into());
    let cerebras = InferenceProvider::Named("cerebras".into());
    let cloudflare = InferenceProvider::Named("cloudflare".into());
    let deepseek_web = InferenceProvider::Named("deepseek-web".into());

    assert!(
        default_provider_budget_rank(&opencode)
            < default_provider_budget_rank(&longcat)
    );
    assert!(
        default_provider_budget_rank(&longcat)
            < default_provider_budget_rank(&mistral)
    );
    assert!(
        default_provider_budget_rank(&mistral)
            < default_provider_budget_rank(&InferenceProvider::OpenRouter)
    );
    assert!(
        default_provider_budget_rank(&InferenceProvider::OpenRouter)
            < default_provider_budget_rank(&github)
    );
    assert!(
        default_provider_budget_rank(&github)
            < default_provider_budget_rank(&bazaarlink)
    );
    assert!(
        default_provider_budget_rank(&bazaarlink)
            < default_provider_budget_rank(&groq)
    );
    assert!(
        default_provider_budget_rank(&groq)
            < default_provider_budget_rank(&cerebras)
    );
    assert!(
        default_provider_budget_rank(&cerebras)
            < default_provider_budget_rank(&cloudflare)
    );
    assert!(
        default_provider_budget_rank(&cloudflare)
            < default_provider_budget_rank(&InferenceProvider::GoogleGemini)
    );
    assert!(
        default_provider_budget_rank(&InferenceProvider::GoogleGemini)
            < default_provider_budget_rank(&deepseek_web)
    );
    assert!(
        cost_class_rank(
            CostClass::Free,
            0,
            default_provider_budget_rank(&deepseek_web),
        ) < cost_class_rank(
            CostClass::Paid,
            0,
            default_provider_budget_rank(&InferenceProvider::Anthropic),
        )
    );
    assert!(
        default_provider_budget_rank(&InferenceProvider::Anthropic)
            < default_provider_budget_rank(&InferenceProvider::OpenAI)
    );
    assert!(
        cost_class_rank(
            CostClass::Paid,
            0,
            default_provider_budget_rank(&InferenceProvider::OpenAI),
        ) < cost_class_rank(
            CostClass::PaidBrowser,
            0,
            default_provider_budget_rank(&InferenceProvider::Named(
                "chatgpt-web".into()
            )),
        )
    );
}

#[test]
fn free_api_ranks_before_chatgpt_web() {
    let openrouter = cost_class_rank(CostClass::Free, 0, 1);
    let chatgpt = cost_class_rank(CostClass::PaidBrowser, 0, 0);
    assert!(openrouter < chatgpt);
}

#[test]
fn paid_api_ranks_before_chatgpt_web() {
    let anthropic = cost_class_rank(CostClass::Paid, 0, 0);
    let chatgpt = cost_class_rank(CostClass::PaidBrowser, 0, 0);
    assert!(anthropic < chatgpt);
}

#[test]
fn gemini_free_ranks_before_deepseek_web() {
    let gemini_free = cost_class_rank(CostClass::Free, 0, 15);
    let deepseek_web = cost_class_rank(CostClass::Free, 0, 16);
    assert!(gemini_free < deepseek_web);
}

#[test]
fn deepseek_web_ranks_before_paid_gemini_default() {
    let deepseek_web = cost_class_rank(CostClass::Free, 0, 16);
    let gemini_paid = cost_class_rank(CostClass::Paid, 10, 15);
    assert!(deepseek_web < gemini_paid);
}

#[test]
fn cost_class_beats_json_schema_rank_gap() {
    let free = cost_class_rank(CostClass::Free, 0, 0);
    let paid = cost_class_rank(CostClass::Paid, 0, 0);
    // json_schema_rank tiebreak is a few points; cost-class gap is 200.
    assert_eq!(paid.saturating_sub(free), 200);
}

#[test]
fn ranks_short_cooldown_cheap_provider_before_expensive_provider() {
    let groq_base = cost_class_rank(CostClass::Free, 0, 7);
    let anthropic_base = cost_class_rank(CostClass::Paid, 0, 0);

    assert!(
        effective_budget_rank(
            groq_base,
            Some(Duration::from_secs(2)),
            Duration::from_secs(3),
        ) < effective_budget_rank(anthropic_base, None, Duration::from_secs(3),)
    );
}
