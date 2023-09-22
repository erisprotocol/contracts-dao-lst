#[cfg(feature = "X-osmosis-X")]
pub mod osmosis;

#[cfg(feature = "X-kujira-X")]
pub mod kujira;

#[cfg(feature = "X-kujira-X")]
pub mod types {
    use cosmwasm_std::Decimal;

    use crate::modules::kujira::KujiraModule;
    pub type UsedCustomModule = KujiraModule;

    pub fn init_custom() -> UsedCustomModule {
        UsedCustomModule {
            oracle_price: Decimal::zero(),
        }
    }
}

#[cfg(feature = "X-whitewhale-X")]
pub mod types {
    use cosmwasm_std::Empty;
    use cw_multi_test::FailingModule;

    pub type UsedCustomModule = FailingModule<Empty, Empty, Empty>;

    pub fn init_custom() -> UsedCustomModule {
        UsedCustomModule::default()
    }
}
