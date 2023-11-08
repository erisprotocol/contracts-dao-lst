use cosmwasm_schema::cw_serde;
use cosmwasm_std::{coin, to_binary, Addr, CosmosMsg, Uint128, WasmMsg};
use cw20::Cw20ExecuteMsg;

use crate::error::CustomResult;

#[cw_serde]
pub struct Ginkou {
    pub contract: Addr,
    pub usdc_denom: String,
    pub musdc_addr: Addr,
}

#[cw_serde]
pub enum GinkouExecuteMsg {
    RedeemStable {},
    DepositStable {},
}

impl Ginkou {
    pub fn deposit_usdc(&self, amount: u128) -> CustomResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: self.contract.to_string(),
            funds: vec![coin(amount, self.usdc_denom.clone())],
            msg: to_binary(&GinkouExecuteMsg::DepositStable {})?,
        }))
    }

    pub fn withdraw_msg(&self, amount: Uint128) -> CustomResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: self.musdc_addr.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: self.contract.to_string(),
                amount,
                msg: to_binary(&GinkouExecuteMsg::RedeemStable {})?,
            })?,
        }))
    }
}
