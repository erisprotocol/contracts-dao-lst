use astroport::asset::AssetInfoExt;
use cosmwasm_std::{
    coins, to_binary, Addr, Coin, CosmosMsg, Decimal, StdError, StdResult, Uint128, WasmMsg,
};
use eris_chain_shared::chain_trait::ChainInterface;

use crate::{
    adapters::whitewhaledex::WhiteWhalePair,
    custom_execute_msg::CustomExecuteMsg,
    types::{CoinType, CustomMsgType, DenomType, MultiSwapRouterType, StageType, WithdrawType},
};

pub struct Chain {
    pub contract: Addr,
}

impl
    ChainInterface<CustomMsgType, DenomType, CoinType, WithdrawType, StageType, MultiSwapRouterType>
    for Chain
{
    fn create_denom_msg(&self, _full_denom: String, subdenom: String) -> CosmosMsg<CustomMsgType> {
        // MsgCreateDenom {
        //     sender: self.contract.to_string(),
        //     subdenom,
        // }
        // .into()

        CosmosMsg::Custom(CustomExecuteMsg::Token(
            crate::custom_execute_msg::TokenExecuteMsg::CreateDenom {
                subdenom,
            },
        ))
    }

    fn create_mint_msgs(
        &self,
        full_denom: String,
        amount: Uint128,
        recipient: Addr,
    ) -> Vec<CosmosMsg<CustomMsgType>> {
        vec![
            CosmosMsg::Custom(CustomExecuteMsg::Token(
                crate::custom_execute_msg::TokenExecuteMsg::MintTokens {
                    denom: full_denom.clone(),
                    amount,
                    mint_to_address: self.contract.to_string(),
                },
            )),
            // MsgMint {
            //     sender: self.contract.to_string(),
            //     amount: Some(terra_proto_rs::cosmos::base::v1beta1::Coin {
            //         denom: full_denom.to_string(),
            //         amount: amount.to_string(),
            //     }),
            //     mint_to_address: self.contract.to_string(),
            // }
            // .into(),
            CosmosMsg::Bank(cosmwasm_std::BankMsg::Send {
                to_address: recipient.to_string(),
                amount: coins(amount.u128(), full_denom),
            }),
        ]
    }

    fn create_burn_msg(&self, full_denom: String, amount: Uint128) -> CosmosMsg<CustomMsgType> {
        // MsgBurn {
        //     sender: self.contract.to_string(),
        //     amount: Some(terra_proto_rs::cosmos::base::v1beta1::Coin {
        //         denom: full_denom,
        //         amount: amount.to_string(),
        //     }),
        //     burn_from_address: self.contract.to_string(),
        // }
        // .into()

        CosmosMsg::Custom(CustomExecuteMsg::Token(
            crate::custom_execute_msg::TokenExecuteMsg::BurnTokens {
                denom: full_denom,
                amount,
                burn_from_address: self.contract.to_string(),
            },
        ))
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

    fn create_multi_swap_router_msgs(
        &self,
        router_type: MultiSwapRouterType,
        assets: Vec<CoinType>,
    ) -> StdResult<Vec<CosmosMsg<CustomMsgType>>> {
        let funds: Vec<Coin> =
            assets.iter().map(|asset| asset.to_coin()).collect::<StdResult<_>>()?;

        match router_type {
            MultiSwapRouterType::Manta {
                addr,
                msg,
            } => Ok(vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                funds,
                msg: to_binary(&msg)?,
            })]),
        }
    }

    fn equals_asset_info(
        &self,
        denom: &DenomType,
        asset_info: &astroport::asset::AssetInfo,
    ) -> bool {
        denom == asset_info
    }

    fn get_coin(&self, denom: DenomType, amount: Uint128) -> CoinType {
        denom.with_balance(amount)
    }
}
