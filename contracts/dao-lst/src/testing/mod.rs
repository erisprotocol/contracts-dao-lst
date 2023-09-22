mod custom_querier;
mod helpers;
pub mod tests_claim;
mod tests_default;
pub mod tests_exchange_rates;

pub use helpers::WithoutGeneric;

#[cfg(feature = "X-kujira-X")]
pub mod tests_kujira;
#[cfg(feature = "X-terra-X")]
pub mod tests_terra;
