use compact_str::CompactString;

use crate::{
    metrics::router::base_router_kv,
    types::{extensions::RouterRuntimeLabels, router::RouterId},
};

#[test]
fn base_router_kv_contains_expected_keys() {
    let rtl = RouterRuntimeLabels {
        router_id: RouterId::Named(CompactString::new("test-router")),
        endpoint_type: "chat".to_string(),
        strategy: "provider-weighted",
    };
    let kv = base_router_kv(&rtl);
    assert_eq!(kv.len(), 3);
    let keys: Vec<_> = kv.iter().map(|k| k.key.as_str()).collect();
    assert!(keys.contains(&"router_id"));
    assert!(keys.contains(&"endpoint_type"));
    assert!(keys.contains(&"strategy"));
}
