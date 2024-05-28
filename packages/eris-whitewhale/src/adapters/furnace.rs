use crate::types::{CustomMsgType, DenomType};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{coins, to_binary, Addr, CosmosMsg, StdError, StdResult, Uint128, WasmMsg};

#[cw_serde]
pub enum ExecuteMsg {
    Burn {},
}

#[cw_serde]
pub struct Furnace(pub Addr);

impl Furnace {
    pub fn burn_msg(
        &self,
        denom: DenomType,
        amount: Uint128,
    ) -> StdResult<CosmosMsg<CustomMsgType>> {
        match denom {
            cw_asset::AssetInfoBase::Native(native) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: self.0.to_string(),
                funds: coins(amount.u128(), native.clone()),
                msg: to_binary(&ExecuteMsg::Burn {})?,
            })),
            _ => Err(StdError::generic_err("Furnace.burn_msg: not supported")),
        }
    }
}
