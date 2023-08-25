use astroport::asset::{Asset, AssetInfo};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, Coin, Empty, StdResult, Uint128};
use eris_chain_shared::chain_trait::Validateable;

#[cw_serde]
pub enum WithdrawType {
    Dex {
        addr: Addr,
    },
}

impl WithdrawType {
    pub fn dex(addr: &str) -> Self {
        Self::Dex {
            addr: Addr::unchecked(addr),
        }
    }
}

#[cw_serde]
pub enum StageType {
    Dex {
        addr: Addr,
    },
    Manta {
        addr: Addr,
        msg: MantaMsg,
    },
}

#[cw_serde]
pub struct MantaMsg {
    pub swap: Swap,
}

#[cw_serde]
pub struct Swap {
    pub stages: Vec<Vec<(String, String)>>,
    pub min_return: Vec<Coin>,
}

impl StageType {
    pub fn dex(addr: &str) -> Self {
        Self::Dex {
            addr: Addr::unchecked(addr),
        }
    }
}

pub type DenomType = AssetInfo;
pub type CustomMsgType = Empty;
pub type CustomQueryType = Empty;
pub type CoinType = Asset;

#[cw_serde]
pub struct HubChainConfigInput {}

impl Validateable<HubChainConfig> for HubChainConfigInput {
    fn validate(&self, _api: &dyn Api) -> StdResult<HubChainConfig> {
        Ok(HubChainConfig {})
    }
}
#[cw_serde]
pub struct HubChainConfig {}

pub fn get_asset(info: AssetInfo, amount: Uint128) -> Asset {
    Asset {
        info,
        amount,
    }
}
