use cosmwasm_std::{
    coins, to_binary, Addr, CosmosMsg, Decimal, StdError, StdResult, Uint128, WasmMsg,
};
use eris_chain_shared::chain_trait::ChainInterface;

use crate::{
    adapters::whitewhaledex::WhiteWhalePair,
    denom::{MsgBurn, MsgCreateDenom, MsgMint},
    types::{CustomMsgType, DenomType, HubChainConfig, StageType, WithdrawType},
};

pub struct Chain {
    pub contract: Addr,
}

impl ChainInterface<CustomMsgType, DenomType, WithdrawType, StageType, HubChainConfig> for Chain {
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
        vec![
            MsgMint {
                sender: self.contract.to_string(),
                amount: Some(crate::denom::Coin {
                    denom: full_denom.to_string(),
                    amount: amount.to_string(),
                }),
            }
            .into(),
            CosmosMsg::Bank(cosmwasm_std::BankMsg::Send {
                to_address: recipient.to_string(),
                amount: coins(amount.u128(), full_denom),
            }),
        ]
    }

    fn create_burn_msg(&self, full_denom: String, amount: Uint128) -> CosmosMsg<CustomMsgType> {
        MsgBurn {
            sender: self.contract.to_string(),
            amount: Some(crate::denom::Coin {
                denom: full_denom,
                amount: amount.to_string(),
            }),
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
            StageType::Manta {
                addr,
                msg,
            } => match denom {
                astroport::asset::AssetInfo::Token {
                    ..
                } => Err(StdError::generic_err("not supported by mnta")),
                astroport::asset::AssetInfo::NativeToken {
                    denom,
                } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: addr.to_string(),
                    funds: coins(amount.u128(), denom),
                    msg: to_binary(&msg)?,
                })),
            },
        }
    }
}
