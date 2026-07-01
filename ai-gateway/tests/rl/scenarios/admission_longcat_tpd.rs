use ai_gateway::{
    config::{
        catalog_limit_resolve::catalog_limit_resolve,
        provider_limits::{ProviderLimitCatalog, QuotaValue},
    },
    tests::routing::{PacingGate, PacingLimits},
    types::provider::InferenceProvider,
};

const MODEL: &str = "LongCat-2.0";

pub async fn run() {
    let catalog = ProviderLimitCatalog::default();
    let provider = InferenceProvider::Named("longcat".into());
    let resolved = catalog_limit_resolve(&catalog, &provider, "free", MODEL)
        .expect("longcat catalog limits");
    let tpd = match resolved.limits.tpd {
        QuotaValue::Limited(value) => {
            u32::try_from(value).expect("tpd fits u32")
        }
        other => panic!("expected catalog tpd limit, got {other:?}"),
    };
    let limits =
        PacingLimits::from_quota(&resolved.limits).expect("pacing limits");
    assert_eq!(limits.tpd, Some(tpd));

    let gate = PacingGate::new(limits);
    let chunk = 1_000u32;
    for _ in 0..(tpd / chunk) {
        let _permit = gate.acquire(chunk).await.expect("catalog fill");
    }
    assert!(
        !gate.daily_headroom_available(chunk).await,
        "catalog TPD must be exhausted without magic constants"
    );
    let wait = gate.daily_reset_wait().await;
    assert!(wait > std::time::Duration::ZERO);
}
