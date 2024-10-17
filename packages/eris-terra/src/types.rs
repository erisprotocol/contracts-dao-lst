use astroport::asset::{Asset, AssetInfo};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, Empty, Uint128};

use crate::custom_execute_msg::CustomExecuteMsg;

pub use astroport::asset::AssetInfoExt;

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
pub enum MultiSwapRouterType {
    Manta {
        addr: Addr,
        msg: MantaMsg,
    },
}

#[cw_serde]
pub struct MantaMsg {
    pub swap: MantaSwap,
}

#[cw_serde]
pub struct MantaSwap {
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
pub type CoinType = Asset;
pub type CustomMsgType = CustomExecuteMsg;
pub type CustomQueryType = Empty;

pub fn get_asset(info: DenomType, amount: Uint128) -> CoinType {
    Asset {
        info,
        amount,
    }
}
