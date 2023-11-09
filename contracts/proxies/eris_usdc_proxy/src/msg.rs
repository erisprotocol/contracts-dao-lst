use astroport::asset::{Asset, AssetInfo, PairInfo};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{to_binary, Addr, CosmosMsg, Decimal, StdResult, WasmMsg};
use eris::adapters::alliancehub::AllianceHub;

use crate::ginkou::Ginkou;

#[cw_serde]
pub struct InstantiateMsg {
    pub ginkou: String,
    pub usdc_denom: String,
    pub musdc_addr: String,

    pub hub: String,
    pub stake: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Bond specified amount of Token
    Bond {
        receiver: Option<String>,
        donate: Option<bool>,
    },

    /// Submit an unbonding request to the current unbonding queue; automatically invokes `unbond`
    /// if `epoch_time` has elapsed since when the last unbonding queue was executed.
    Unbond {
        receiver: Option<String>,
    },

    /// Same as bond / unbond, belief_price, max_spread are ignored
    Swap {
        offer_asset: Asset,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
        to: Option<String>,
    },

    Callback(CallbackMsg),
}

#[cw_serde]
pub enum CallbackMsg {
    Bond {
        receiver: Addr,
        donate: Option<bool>,
    },
    Unbond {},
    SendTo {
        to: Addr,
        asset_info: AssetInfo,
    },
}

impl CallbackMsg {
    pub fn into_cosmos_msg(&self, contract_addr: &Addr) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_addr.to_string(),
            msg: to_binary(&ExecuteMsg::Callback(self.clone()))?,
            funds: vec![],
        }))
    }
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    Config {},
    #[returns(PairInfo)]
    Pair {},
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct Config {
    pub ginkou: Ginkou,
    pub hub: AllianceHub,
    pub stake: String,
}
