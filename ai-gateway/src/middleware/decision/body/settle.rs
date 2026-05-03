use std::sync::Arc;

use crate::middleware::decision::budget::StateStore;

pub(super) fn commit(
    store: Arc<dyn StateStore>,
    key: String,
    reservation_id: String,
    amount: i64,
) {
    tokio::spawn(async move {
        if let Err(error) = store
            .commit_reservation(&key, &reservation_id, amount)
            .await
        {
            tracing::warn!(%error, "failed to commit decision reservation");
        }
    });
}

pub(super) fn refund(
    store: Arc<dyn StateStore>,
    key: String,
    reservation_id: String,
) {
    tokio::spawn(async move {
        if let Err(error) =
            store.refund_reservation(&key, &reservation_id).await
        {
            tracing::warn!(%error, "failed to refund decision reservation");
        }
    });
}
