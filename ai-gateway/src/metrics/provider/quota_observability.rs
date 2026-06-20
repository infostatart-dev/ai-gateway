use std::{str::FromStr, time::Instant};

use chrono::{DateTime, Utc};

use super::runtime::{ProviderStatsRow, ProviderStatsSnapshot};
use crate::{
    app_state::AppState,
    config::provider_limits::ProviderQuotaProfile,
    router::quota_admission::{
        BlockedReason, PacingAdmissionScope, evaluate_pacing_admission,
    },
    types::provider::InferenceProvider,
};

#[derive(Debug, Clone, serde::Serialize)]
pub struct QuotaModelRow {
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_available_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<BlockedReason>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct QuotaAccountRow {
    pub credential_id: String,
    pub quota_profile: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_available_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<BlockedReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<QuotaModelRow>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct QuotaProviderRow {
    pub provider: String,
    pub accounts: Vec<QuotaAccountRow>,
}

pub async fn enrich_snapshot(
    app_state: &AppState,
    mut snapshot: ProviderStatsSnapshot,
) -> ProviderStatsSnapshot {
    let now = Instant::now();
    let pacing = app_state.upstream_pacing();
    let health = app_state.credential_health();
    let limits = &app_state.config().provider_limits;
    let credentials = &app_state.config().credentials;

    for row in &mut snapshot.providers {
        let provider =
            InferenceProvider::from_str(&row.provider).expect("infallible");
        let cred = credentials
            .get(&crate::config::credentials::ProviderCredentialId::new(
                &row.credential,
            ))
            .or_else(|| {
                credentials
                    .for_provider(&provider)
                    .find(|c| c.id.as_str() == row.credential)
            });
        let tier = cred.map_or("default", |c| c.tier.as_str());
        let profile = limits.quota_profile(&provider);
        row.quota_profile = Some(quota_profile_label(profile).to_string());

        match profile {
            ProviderQuotaProfile::PerModel => {
                let models = catalog_models(limits, &provider, tier);
                let mut model_rows = Vec::new();
                for slug in models {
                    let verdict = evaluate_pacing_admission(PacingAdmissionScope {
                        pacing,
                        health,
                        limits,
                        provider: &provider,
                        credential_id: &crate::config::credentials::ProviderCredentialId::new(
                            &row.credential,
                        ),
                        tier,
                        model: Some(slug.as_str()),
                        estimated_tokens: 0,
                        now,
                    })
                    .await;
                    model_rows.push(quota_model_row(slug, &verdict));
                }
                row.models = Some(model_rows);
            }
            ProviderQuotaProfile::PerSlot
            | ProviderQuotaProfile::PerSession => {
                let verdict = evaluate_pacing_admission(PacingAdmissionScope {
                    pacing,
                    health,
                    limits,
                    provider: &provider,
                    credential_id:
                        &crate::config::credentials::ProviderCredentialId::new(
                            &row.credential,
                        ),
                    tier,
                    model: None,
                    estimated_tokens: 0,
                    now,
                })
                .await;
                apply_verdict(row, &verdict);
            }
        }
    }

    snapshot.quota = build_quota_tree(&snapshot.providers);
    snapshot
}

fn quota_model_row(
    slug: String,
    verdict: &crate::router::quota_admission::AdmissionVerdict,
) -> QuotaModelRow {
    QuotaModelRow {
        slug,
        next_available_at: verdict.next_available_at,
        blocked_reason: blocked_reason_field(verdict),
    }
}

fn apply_verdict(
    row: &mut ProviderStatsRow,
    verdict: &crate::router::quota_admission::AdmissionVerdict,
) {
    row.next_available_at = verdict.next_available_at;
    row.blocked_reason = blocked_reason_field(verdict);
}

fn blocked_reason_field(
    verdict: &crate::router::quota_admission::AdmissionVerdict,
) -> Option<BlockedReason> {
    if verdict.blocked_reason == BlockedReason::None {
        None
    } else {
        Some(verdict.blocked_reason)
    }
}

fn quota_profile_label(profile: ProviderQuotaProfile) -> &'static str {
    match profile {
        ProviderQuotaProfile::PerModel => "per-model",
        ProviderQuotaProfile::PerSlot => "per-slot",
        ProviderQuotaProfile::PerSession => "per-session",
    }
}

fn catalog_models(
    limits: &crate::config::provider_limits::ProviderLimitCatalog,
    provider: &InferenceProvider,
    tier: &str,
) -> Vec<String> {
    limits
        .provider(provider)
        .and_then(|config| config.tier(tier))
        .map(|tier_config| tier_config.models.keys().cloned().collect())
        .unwrap_or_default()
}

fn build_quota_tree(rows: &[ProviderStatsRow]) -> Vec<QuotaProviderRow> {
    use std::collections::BTreeMap;

    let mut by_provider: BTreeMap<String, Vec<QuotaAccountRow>> =
        BTreeMap::new();
    for row in rows {
        by_provider.entry(row.provider.clone()).or_default().push(
            QuotaAccountRow {
                credential_id: row.credential.clone(),
                quota_profile: row.quota_profile.clone().unwrap_or_default(),
                next_available_at: row.next_available_at,
                blocked_reason: row.blocked_reason,
                models: row.models.clone(),
            },
        );
    }
    by_provider
        .into_iter()
        .map(|(provider, accounts)| QuotaProviderRow { provider, accounts })
        .collect()
}
