use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Empty, Uint128};
use cw_asset::{Asset, AssetInfo};

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
    Burn {
        addr: Addr,
    },
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
pub type CoinType = Asset;
pub type CustomQueryType = Empty;
pub type MultiSwapRouterType = Empty;

pub fn get_asset(info: DenomType, amount: Uint128) -> CoinType {
    Asset {
        info,
        amount,
    }
}

pub trait AssetInfoExt {
    /// simplifies converting an AssetInfo to an Asset with balance
    fn with_balance(&self, balance: Uint128) -> Asset;
}

impl AssetInfoExt for AssetInfo {
    fn with_balance(&self, amount: Uint128) -> Asset {
        match self {
            cw_asset::AssetInfoBase::Native(denom) => Asset::native(denom, amount),
            cw_asset::AssetInfoBase::Cw20(contract_addr) => {
                Asset::cw20(contract_addr.clone(), amount)
            },
            _ => todo!(),
        }
    }
}
