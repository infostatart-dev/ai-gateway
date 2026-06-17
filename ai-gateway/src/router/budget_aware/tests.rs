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
    let openrouter = InferenceProvider::OpenRouter;
    let github = InferenceProvider::Named("github-models".into());
    let mistral = InferenceProvider::Named("mistral".into());
    let groq = InferenceProvider::Named("groq".into());
    let cerebras = InferenceProvider::Named("cerebras".into());
    let cloudflare = InferenceProvider::Named("cloudflare".into());
    let gemini = InferenceProvider::GoogleGemini;
    let deepseek_web = InferenceProvider::Named("deepseek-web".into());
    let anthropic = InferenceProvider::Anthropic;
    let openai = InferenceProvider::OpenAI;
    let chatgpt_web = InferenceProvider::Named("chatgpt-web".into());
    let longcat = InferenceProvider::Named("longcat".into());

    assert!(
        default_provider_budget_rank(&opencode)
            < default_provider_budget_rank(&openrouter)
    );
    assert!(
        default_provider_budget_rank(&openrouter)
            < default_provider_budget_rank(&github)
    );
    assert!(
        default_provider_budget_rank(&github)
            < default_provider_budget_rank(&mistral)
    );
    assert!(
        default_provider_budget_rank(&mistral)
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
            < default_provider_budget_rank(&gemini)
    );
    assert!(
        default_provider_budget_rank(&gemini)
            < default_provider_budget_rank(&deepseek_web)
    );
    assert!(
        default_provider_budget_rank(&deepseek_web)
            < default_provider_budget_rank(&anthropic)
    );
    assert!(
        default_provider_budget_rank(&anthropic)
            < default_provider_budget_rank(&openai)
    );
    assert!(
        default_provider_budget_rank(&openai)
            < default_provider_budget_rank(&chatgpt_web)
    );
    assert!(
        default_provider_budget_rank(&chatgpt_web)
            < default_provider_budget_rank(&longcat)
    );
}

#[test]
fn free_api_ranks_before_chatgpt_web() {
    let openrouter = cost_class_rank(CostClass::Free, 0, 1);
    let chatgpt = cost_class_rank(CostClass::PaidBrowser, 0, 11);
    assert!(openrouter < chatgpt);
}

#[test]
fn paid_api_ranks_before_chatgpt_web() {
    let anthropic = cost_class_rank(CostClass::Paid, 0, 9);
    let chatgpt = cost_class_rank(CostClass::PaidBrowser, 0, 11);
    assert!(anthropic < chatgpt);
}

#[test]
fn gemini_free_ranks_before_deepseek_web() {
    let gemini_free = cost_class_rank(CostClass::Free, 0, 7);
    let deepseek_web = cost_class_rank(CostClass::Free, 0, 8);
    assert!(gemini_free < deepseek_web);
}

#[test]
fn deepseek_web_ranks_before_paid_gemini_default() {
    let deepseek_web = cost_class_rank(CostClass::Free, 0, 8);
    let gemini_paid = cost_class_rank(CostClass::Paid, 10, 7);
    assert!(deepseek_web < gemini_paid);
}

#[test]
fn cost_class_beats_json_schema_rank_gap() {
    let free = cost_class_rank(CostClass::Free, 0, 0);
    let paid = cost_class_rank(CostClass::Paid, 0, 0);
    assert_eq!(paid.saturating_sub(free), 200);
}

#[test]
fn ranks_short_cooldown_cheap_provider_before_expensive_provider() {
    let groq_base = cost_class_rank(CostClass::Free, 0, 4);
    let anthropic_base = cost_class_rank(CostClass::Paid, 0, 9);

    assert!(
        effective_budget_rank(
            groq_base,
            Some(Duration::from_secs(2)),
            Duration::from_secs(3),
        ) < effective_budget_rank(anthropic_base, None, Duration::from_secs(3),)
    );
}
