pub mod harness;
pub mod mock;

#[cfg(feature = "testing")]
pub mod budget_aware;
#[cfg(feature = "testing")]
pub mod routing;
#[cfg(feature = "testing")]
pub mod routing_harness;

pub trait TestDefault {
    fn test_default() -> Self;
}
