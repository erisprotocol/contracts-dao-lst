use astroport::asset::AssetInfo;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    coin, to_binary, CosmosMsg, Empty, Env, StdError, StdResult, Uint128, VoteOption, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use eris_chain_adapter::types::CustomMsgType;

use crate::hub::{DaoInterface, StakeToken};

#[cw_serde]
pub enum EnterpriseCw20HookMsg {
    Stake {},
}

#[cw_serde]
pub enum EnterpriseExecuteMsg {
    CastVote(CastVoteMsg),
    Unstake(EnterpriseUnstakeMsg),
    Claim {},
}

#[cw_serde]
pub struct CastVoteMsg {
    pub proposal_id: u64,
    pub outcome: VoteOutcome,
}

#[cw_serde]
#[repr(u8)]
// TODO: rename to VoteOption?
pub enum VoteOutcome {
    Yes = 0,
    No = 1,
    Abstain = 2,
    Veto = 3,
}

impl From<u8> for VoteOutcome {
    fn from(v: u8) -> VoteOutcome {
        match v {
            0u8 => VoteOutcome::Yes,
            1u8 => VoteOutcome::No,
            2u8 => VoteOutcome::Abstain,
            3u8 => VoteOutcome::Veto,
            _ => panic!("invalid vote option"),
        }
    }
}

#[cw_serde]
pub enum EnterpriseUnstakeMsg {
    Cw20(EnterpriseUnstakeCw20Msg),
}

#[cw_serde]
pub struct EnterpriseUnstakeCw20Msg {
    pub amount: Uint128,
}

#[cw_serde]
pub enum FundDistributorExecuteMsg {
    ClaimRewards(ClaimRewardsMsg),
}

#[cw_serde]
pub struct ClaimRewardsMsg {
    pub user: String,
    /// Native denominations to be claimed
    pub native_denoms: Option<Vec<String>>,
    /// CW20 asset rewards to be claimed, should be addresses of CW20 tokens
    pub cw20_assets: Option<Vec<String>>,
}

impl StakeToken {
    pub fn deposit_msg(&self, amount: Uint128) -> StdResult<CosmosMsg<CustomMsgType>> {
        match &self.utoken {
            AssetInfo::Token {
                contract_addr,
            } => match &self.dao_interface {
                DaoInterface::Enterprise {
                    addr,
                    ..
                } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Send {
                        contract: addr.to_string(),
                        amount,
                        msg: to_binary(&EnterpriseCw20HookMsg::Stake {})?,
                    })?,
                    funds: vec![],
                })),

                DaoInterface::CW4 {
                    addr,
                    ..
                } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Send {
                        contract: addr.to_string(),
                        amount,
                        msg: to_binary(&cw4_stake::msg::ReceiveMsg::Bond {})?,
                    })?,
                    funds: vec![],
                })),
            },
            AssetInfo::NativeToken {
                denom,
            } => match &self.dao_interface {
                DaoInterface::Enterprise {
                    ..
                } => Err(StdError::generic_err("native_token not supported for enterprise")),
                DaoInterface::CW4 {
                    addr,
                    ..
                } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: addr.to_string(),
                    msg: to_binary(&cw4_stake::msg::ExecuteMsg::Bond {})?,
                    funds: vec![coin(amount.u128(), denom)],
                })),
            },
        }
    }

    pub fn unbond_msg(&self, amount: Uint128) -> StdResult<CosmosMsg<CustomMsgType>> {
        match &self.dao_interface {
            DaoInterface::Enterprise {
                addr,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                msg: to_binary(&EnterpriseExecuteMsg::Unstake(EnterpriseUnstakeMsg::Cw20(
                    EnterpriseUnstakeCw20Msg {
                        amount,
                    },
                )))?,
                funds: vec![],
            })),
            DaoInterface::CW4 {
                addr,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                msg: to_binary(&cw4_stake::msg::ExecuteMsg::Unbond {
                    tokens: amount,
                })?,
                funds: vec![],
            })),
        }
    }

    pub fn claim_unbonded_msg(&self) -> StdResult<CosmosMsg<CustomMsgType>> {
        match &self.dao_interface {
            DaoInterface::Enterprise {
                addr,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                msg: to_binary(&EnterpriseExecuteMsg::Claim {})?,
                funds: vec![],
            })),
            DaoInterface::CW4 {
                addr,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                msg: to_binary(&cw4_stake::msg::ExecuteMsg::Claim {})?,
                funds: vec![],
            })),
        }
    }

    pub fn claim_rewards_msg(
        &self,
        env: &Env,
        native_denoms: Vec<String>,
        cw20_assets: Vec<String>,
    ) -> StdResult<CosmosMsg<CustomMsgType>> {
        match &self.dao_interface {
            DaoInterface::Enterprise {
                fund_distributor,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: fund_distributor.to_string(),
                msg: to_binary(&FundDistributorExecuteMsg::ClaimRewards(ClaimRewardsMsg {
                    user: env.contract.address.to_string(),
                    native_denoms: Some(native_denoms),
                    cw20_assets: Some(cw20_assets),
                }))?,
                funds: vec![],
            })),
            DaoInterface::CW4 {
                fund_distributor,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: fund_distributor.to_string(),
                msg: to_binary(&FundDistributorExecuteMsg::ClaimRewards(ClaimRewardsMsg {
                    user: env.contract.address.to_string(),
                    native_denoms: None,
                    cw20_assets: None,
                }))?,
                funds: vec![],
            })),
        }
    }

    pub fn vote_msg(
        &self,
        proposal_id: u64,
        outcome: VoteOption,
    ) -> StdResult<CosmosMsg<CustomMsgType>> {
        match &self.dao_interface {
            DaoInterface::Enterprise {
                addr,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                msg: to_binary(&EnterpriseExecuteMsg::CastVote(CastVoteMsg {
                    proposal_id,
                    outcome: match outcome {
                        VoteOption::Yes => VoteOutcome::Yes,
                        VoteOption::No => VoteOutcome::No,
                        VoteOption::Abstain => VoteOutcome::Abstain,
                        VoteOption::NoWithVeto => VoteOutcome::Veto,
                    },
                }))?,
                funds: vec![],
            })),
            DaoInterface::CW4 {
                gov,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: gov.to_string(),
                msg: to_binary(&cw3::Cw3ExecuteMsg::Vote::<Empty> {
                    proposal_id,
                    vote: match outcome {
                        VoteOption::Yes => cw3::Vote::Yes,
                        VoteOption::No => cw3::Vote::No,
                        VoteOption::Abstain => cw3::Vote::Abstain,
                        VoteOption::NoWithVeto => cw3::Vote::Veto,
                    },
                })?,
                funds: vec![],
            })),
        }
    }
}
