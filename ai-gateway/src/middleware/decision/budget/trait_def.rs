use std::fmt;

#[async_trait::async_trait]
pub trait StateStore: Send + Sync + fmt::Debug {
    async fn reserve(&self, key: &str, amount: i64) -> Result<String, String>;

    async fn commit_reservation(
        &self,
        key: &str,
        reservation_id: &str,
        final_amount: i64,
    ) -> Result<(), String>;

    async fn refund_reservation(
        &self,
        key: &str,
        reservation_id: &str,
    ) -> Result<(), String>;
}
