pub mod clock;
pub mod memory;
pub mod redis;
pub mod store;

pub use clock::{QuotaClock, QuotaWindow};
pub use memory::MemoryClientAccessQuotaStore;
pub use redis::RedisClientAccessQuotaStore;
pub use store::{
    ClientAccessQuotaStore, QuotaAdmission, QuotaAdmissionError,
    QuotaDimension, QuotaFamily, QuotaLimitStatus, QuotaRejection,
    QuotaReservation, QuotaStoreError, QuotaWindowKind,
};

pub fn build_quota_store(
    config: &crate::config::client_access::ClientAccessQuotaStoreConfig,
) -> Result<
    std::sync::Arc<dyn ClientAccessQuotaStore>,
    crate::error::init::InitError,
> {
    match config {
        crate::config::client_access::ClientAccessQuotaStoreConfig::Memory => {
            Ok(std::sync::Arc::new(MemoryClientAccessQuotaStore::new()))
        }
        crate::config::client_access::ClientAccessQuotaStoreConfig::Redis(
            redis,
        ) => Ok(std::sync::Arc::new(
            RedisClientAccessQuotaStore::from_config(redis)?,
        )),
    }
}
