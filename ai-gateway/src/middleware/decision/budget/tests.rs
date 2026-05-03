use super::{MemoryStateStore, StateStore};

#[tokio::test]
async fn memory_store_commits_final_amount() {
    let store = MemoryStateStore::new();
    let reservation = store.reserve("user", 100).await.unwrap();

    store
        .commit_reservation("user", &reservation, 40)
        .await
        .unwrap();

    assert_eq!(store.usage_snapshot("user"), 40);
}

#[tokio::test]
async fn memory_store_refunds_reserved_amount() {
    let store = MemoryStateStore::new();
    let reservation = store.reserve("user", 100).await.unwrap();

    store
        .refund_reservation("user", &reservation)
        .await
        .unwrap();

    assert_eq!(store.usage_snapshot("user"), 0);
}
