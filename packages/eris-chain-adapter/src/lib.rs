#[cfg(feature = "X-kujira-X")]
pub mod types {
    use cosmwasm_std::DepsMut;
    use cosmwasm_std::Env;
    use cosmwasm_std::StdResult;
    use cosmwasm_std::Uint128;
    use std::collections::HashMap;

    use eris_chain_shared::chain_trait::ChainInterface;

    pub use eris_kujira::chain::KujiraChain;
    pub use eris_kujira::types::AssetInfoExt;
    pub use eris_kujira::types::CoinType;
    pub use eris_kujira::types::CustomMsgType;
    pub use eris_kujira::types::CustomQueryType;
    pub use eris_kujira::types::DenomType;
    pub use eris_kujira::types::HubChainConfig;
    pub use eris_kujira::types::HubChainConfigInput;
    pub use eris_kujira::types::MantaMsg;
    pub use eris_kujira::types::MantaSwap;
    pub use eris_kujira::types::MultiSwapRouterType;
    pub use eris_kujira::types::StageType;
    pub use eris_kujira::types::WithdrawType;

    pub const CHAIN_TYPE: &str = "kujira";

    #[inline(always)]
    pub fn chain(
        _env: &Env,
    ) -> impl ChainInterface<
        CustomMsgType,
        DenomType,
        CoinType,
        WithdrawType,
        StageType,
        MultiSwapRouterType,
    > {
        KujiraChain {}
    }

    /// queries all balances and converts it to a hashmap
    pub fn get_balances_hashmap<F>(
        deps: &DepsMut<CustomQueryType>,
        env: Env,
        _get_denoms: F,
    ) -> StdResult<HashMap<String, Uint128>>
    where
        F: FnOnce() -> Vec<DenomType>,
    {
        let balances = deps.querier.query_all_balances(env.contract.address)?;
        let balances: HashMap<_, _> =
            balances.into_iter().map(|item| (item.denom.clone(), item.amount)).collect();
        Ok(balances)
    }
}

#[cfg(feature = "X-whitewhale-X")]
pub mod types {
    use cosmwasm_std::DepsMut;
    use cosmwasm_std::Empty;
    use cosmwasm_std::Env;
    use cosmwasm_std::StdError;
    use cosmwasm_std::StdResult;
    use cosmwasm_std::Uint128;
    use eris_whitewhale::chain::Chain;
    use std::collections::HashMap;

    use eris_chain_shared::chain_trait::ChainInterface;

    pub use eris_whitewhale::types::get_asset;
    pub use eris_whitewhale::types::AssetInfoExt;
    pub use eris_whitewhale::types::CoinType;
    pub use eris_whitewhale::types::CustomMsgType;
    pub use eris_whitewhale::types::CustomQueryType;
    pub use eris_whitewhale::types::DenomType;
    pub use eris_whitewhale::types::MultiSwapRouterType;
    pub use eris_whitewhale::types::StageType;
    pub use eris_whitewhale::types::WithdrawType;

    pub const CHAIN_TYPE: &str = "migaloo";

    #[inline(always)]
    pub fn chain(
        env: &Env,
    ) -> impl ChainInterface<CustomMsgType, DenomType, CoinType, WithdrawType, StageType, Empty>
    {
        Chain {
            contract: env.contract.address.clone(),
        }
    }

    /// queries all balances and converts it to a hashmap
    pub fn get_balances_hashmap<F>(
        deps: &DepsMut<CustomQueryType>,
        env: Env,
        get_denoms: F,
    ) -> StdResult<HashMap<String, Uint128>>
    where
        F: FnOnce() -> Vec<DenomType>,
    {
        let balances: HashMap<_, _> = get_denoms()
            .into_iter()
            .map(|denom| {
                let balance = denom
                    .query_balance(&deps.querier, env.contract.address.clone())
                    .map_err(|e| StdError::generic_err(e.to_string()))?;

                Ok(get_asset(denom, balance))
            })
            .collect::<StdResult<Vec<CoinType>>>()?
            .into_iter()
            .map(|element| (element.info.to_string(), element.amount))
            .collect();

        Ok(balances)
    }
}

#[cfg(feature = "X-terra-X")]
pub mod types {
    use cosmwasm_std::DepsMut;
    use cosmwasm_std::Env;
    use cosmwasm_std::StdError;
    use cosmwasm_std::StdResult;
    use cosmwasm_std::Uint128;
    use std::collections::HashMap;

    use eris_chain_shared::chain_trait::ChainInterface;

    use eris_terra::chain::Chain;
    pub use eris_terra::types::get_asset;
    pub use eris_terra::types::AssetInfoExt;
    pub use eris_terra::types::CoinType;
    pub use eris_terra::types::CustomMsgType;
    pub use eris_terra::types::CustomQueryType;
    pub use eris_terra::types::DenomType;
    pub use eris_terra::types::MantaMsg;
    pub use eris_terra::types::MantaSwap;
    pub use eris_terra::types::MultiSwapRouterType;
    pub use eris_terra::types::StageType;
    pub use eris_terra::types::WithdrawType;

    pub const CHAIN_TYPE: &str = "terra";

    #[inline(always)]
    pub fn chain(
        env: &Env,
    ) -> impl ChainInterface<
        CustomMsgType,
        DenomType,
        CoinType,
        WithdrawType,
        StageType,
        MultiSwapRouterType,
    > {
        Chain {
            contract: env.contract.address.clone(),
        }
    }

    /// queries all balances and converts it to a hashmap
    pub fn get_balances_hashmap<F>(
        deps: &DepsMut<CustomQueryType>,
        env: Env,
        get_denoms: F,
    ) -> StdResult<HashMap<String, Uint128>>
    where
        F: FnOnce() -> Vec<DenomType>,
    {
        let balances: HashMap<_, _> = get_denoms()
            .into_iter()
            .map(|denom| {
                let balance = denom
                    .query_pool(&deps.querier, env.contract.address.clone())
                    .map_err(|e| StdError::generic_err(e.to_string()))?;

                Ok(get_asset(denom, balance))
            })
            .collect::<StdResult<Vec<CoinType>>>()?
            .into_iter()
            .map(|element| (element.info.to_string(), element.amount))
            .collect();

        Ok(balances)
    }
}
