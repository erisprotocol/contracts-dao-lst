use astroport::asset::{Asset, AssetInfo, PairInfo};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal};
use cw20::Cw20ReceiveMsg;

use crate::ginkou::Ginkou;

#[cw_serde]
pub struct InstantiateMsg {
    pub ginkou: String,
    pub usdc_denom: String,
    pub musdc_addr: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Implements the Cw20 receiver interface
    Receive(Cw20ReceiveMsg),

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
pub enum ReceiveMsg {
    /// Swap a given amount of asset
    Swap {
        ask_asset_info: Option<AssetInfo>,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,

        to: Option<String>,
    },
}

#[cw_serde]
pub enum CallbackMsg {
    SendTo {
        to: Addr,
        asset_info: AssetInfo,
    },
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
}
