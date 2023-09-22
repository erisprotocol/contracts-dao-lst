use astroport::asset::AssetInfo;
use cosmwasm_std::{Addr, Api, CosmosMsg, Decimal, StdResult, Uint128};

pub trait ChainInterface<
    TCustom,
    TDenomType,
    TCoinType,
    TWithdrawType,
    TStageType,
    TMultiSwapRouterType,
>
{
    fn get_token_denom(&self, contract_addr: impl Into<String>, sub_denom: String) -> String {
        format!("factory/{0}/{1}", contract_addr.into(), sub_denom)
    }

    fn equals_asset_info(&self, denom: &TDenomType, asset_info: &AssetInfo) -> bool;
    fn get_coin(&self, denom: TDenomType, amount: Uint128) -> TCoinType;

    fn create_denom_msg(&self, full_denom: String, sub_denom: String) -> CosmosMsg<TCustom>;
    // this can sometimes be multiple messages to mint + transfer
    fn create_mint_msgs(
        &self,
        full_denom: String,
        amount: Uint128,
        recipient: Addr,
    ) -> Vec<CosmosMsg<TCustom>>;

    fn create_burn_msg(&self, full_denom: String, amount: Uint128) -> CosmosMsg<TCustom>;

    fn create_withdraw_msg(
        &self,
        withdraw_type: TWithdrawType,
        denom: TDenomType,
        amount: Uint128,
    ) -> StdResult<Option<CosmosMsg<TCustom>>>;

    fn create_single_stage_swap_msgs(
        &self,
        stage_type: TStageType,
        denom: TDenomType,
        amount: Uint128,
        belief_price: Option<Decimal>,
        max_spread: Decimal,
    ) -> StdResult<CosmosMsg<TCustom>>;

    fn create_multi_swap_router_msgs(
        &self,
        router_type: TMultiSwapRouterType,
        assets: Vec<TCoinType>,
    ) -> StdResult<Vec<CosmosMsg<TCustom>>>;
}

pub trait Validateable<T> {
    fn validate(&self, api: &dyn Api) -> StdResult<T>;
}
