use astroport::asset::AssetInfo;
use cosmwasm_std::{Addr, CosmosMsg, Decimal, Empty, StdResult, Uint128};
use cw_asset::Asset;
use eris_chain_shared::chain_trait::ChainInterface;

use crate::{
    adapters::{furnace::Furnace, whitewhaledex::WhiteWhalePair},
    denom::{MsgBurn, MsgCreateDenom, MsgMint},
    types::{CoinType, CustomMsgType, DenomType, StageType, WithdrawType},
};

pub struct Chain {
    pub contract: Addr,
}

impl ChainInterface<CustomMsgType, DenomType, CoinType, WithdrawType, StageType, Empty> for Chain {
    fn create_denom_msg(&self, _full_denom: String, subdenom: String) -> CosmosMsg<CustomMsgType> {
        MsgCreateDenom {
            sender: self.contract.to_string(),
            subdenom,
        }
        .into()
    }

    fn create_mint_msgs(
        &self,
        full_denom: String,
        amount: Uint128,
        recipient: Addr,
    ) -> Vec<CosmosMsg<CustomMsgType>> {
        vec![MsgMint {
            sender: self.contract.to_string(),
            amount: Some(crate::denom::Coin {
                denom: full_denom.to_string(),
                amount: amount.to_string(),
            }),
            mint_to_address: recipient.to_string(),
        }
        .into()]
    }

    fn create_burn_msg(&self, full_denom: String, amount: Uint128) -> CosmosMsg<CustomMsgType> {
        MsgBurn {
            sender: self.contract.to_string(),
            amount: Some(crate::denom::Coin {
                denom: full_denom,
                amount: amount.to_string(),
            }),
            burn_from_address: self.contract.to_string(),
        }
        .into()
    }

    fn create_withdraw_msg(
        &self,
        withdraw_type: WithdrawType,
        denom: DenomType,
        amount: Uint128,
    ) -> StdResult<Option<CosmosMsg<CustomMsgType>>> {
        match withdraw_type {
            WithdrawType::Dex {
                addr,
            } => Ok(Some(WhiteWhalePair(addr).withdraw_msg(denom, amount)?)),
        }
    }

    fn create_single_stage_swap_msgs(
        &self,
        stage_type: StageType,
        denom: DenomType,
        amount: Uint128,
        belief_price: Option<Decimal>,
        max_spread: Decimal,
    ) -> StdResult<CosmosMsg<CustomMsgType>> {
        match stage_type {
            StageType::Dex {
                addr,
            } => WhiteWhalePair(addr).swap_msg(denom, amount, belief_price, Some(max_spread)),
            StageType::Burn {
                addr,
            } => Furnace(addr).burn_msg(denom, amount),
        }
    }

    fn create_multi_swap_router_msgs(
        &self,
        _router_type: Empty,
        _assets: Vec<CoinType>,
    ) -> StdResult<Vec<CosmosMsg<CustomMsgType>>> {
        Ok(vec![])
    }

    fn equals_asset_info(&self, denom: &DenomType, asset_info: &AssetInfo) -> bool {
        match denom {
            cw_asset::AssetInfoBase::Native(native) => match asset_info {
                AssetInfo::Token {
                    ..
                } => false,
                AssetInfo::NativeToken {
                    denom,
                } => denom == native,
            },
            cw_asset::AssetInfoBase::Cw20(cw20) => match asset_info {
                AssetInfo::Token {
                    contract_addr,
                } => cw20 == contract_addr,
                AssetInfo::NativeToken {
                    ..
                } => false,
            },
            _ => false,
        }
    }

    fn get_coin(&self, info: DenomType, amount: Uint128) -> CoinType {
        Asset {
            info,
            amount,
        }
    }
}
