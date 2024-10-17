use crate::hub::DaoInterface;
use astroport::asset::AssetInfo;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    coin, to_json_binary, Addr, Binary, CosmosMsg, Empty, Env, QuerierWrapper, StdError, StdResult,
    Uint128, VoteOption, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Expiration};
use eris_chain_adapter::types::CustomMsgType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnterpriseCw20HookMsg {
    Stake {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnterpriseV2Cw20HookMsg {
    Stake {
        user: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnterpriseExecuteMsg {
    CastVote(CastVoteMsg),
    Unstake(EnterpriseUnstakeMsg),
    Claim {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnterpriseExecuteMsgV2 {
    Unstake(EnterpriseUnstakeMsgV2),
    // CastVote(CastVoteMsgV2),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct CastVoteMsg {
    pub proposal_id: u64,
    pub outcome: VoteOutcome,
}

// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
// #[serde(rename_all = "snake_case")]
// pub struct CastVoteMsgV2 {
//     pub poll_id: u64,
//     pub outcome: VoteOutcome,
//     pub voter: Addr,
//     pub amount: Uint128,
// }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnterpriseUnstakeMsg {
    Cw20(EnterpriseUnstakeCw20Msg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct EnterpriseUnstakeMsgV2 {
    pub amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct EnterpriseUnstakeCw20Msg {
    pub amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnterpriseDistributorExecuteMsg {
    ClaimRewards(EnterpriseClaimRewardsMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct EnterpriseClaimRewardsMsg {
    pub user: String,
    /// Native denominations to be claimed
    pub native_denoms: Option<Vec<String>>,
    /// CW20 asset rewards to be claimed, should be addresses of CW20 tokens
    pub cw20_assets: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw4DistributorExecuteMsg {
    ClaimRewards(Cw4ClaimRewardsMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Cw4ClaimRewardsMsg {
    pub user: String,
    /// Native denominations to be claimed
    pub native_denoms: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw3QueryMsg {
    Proposal {
        proposal_id: u64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Cw3ProposalResponse {
    pub id: u64,
    pub expires: Expiration,
}

// /// Unique identifier for a poll.
// pub type PollId = u64;

// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
// #[serde(rename_all = "snake_case")]
// /// Params for querying a poll.
// pub struct PollParams {
//     /// ID of the poll to be queried.
//     pub poll_id: PollId,
// }

// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
// #[serde(rename_all = "snake_case")]
// /// Response model for querying a poll.
// pub struct EnterprisePollResponse {
//     /// The poll.
//     pub poll: Poll,
// }
// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
// #[serde(rename_all = "snake_case")]
// /// A poll.
// pub struct Poll {
//     /// Unique identifier for the poll.
//     pub id: PollId,
//     /// End-time of poll.
//     pub ends_at: Timestamp,
// }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ProposalResponse {
    pub end_time_s: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnterpriseQueryMsg {
    Proposal(ProposalParams),
    // Poll(PollParams),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ProposalParams {
    pub proposal_id: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct EnterpriseProposalResponse {
    pub proposal: Proposal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Proposal {
    pub id: u64,
    pub expires: Expiration,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CapaVoteOption {
    Yes,
    No,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CapaExecuteMsg {
    CastVote {
        poll_id: u64,
        vote: CapaVoteOption,
        amount: Uint128,
    },

    WithdrawVotingTokens {
        amount: Option<Uint128>,
    },

    Claim {},

    /// cw20 callback
    StakeVotingTokens {},
}

#[cw_serde]
pub enum DaoDaoExecuteMsg {
    /// Stakes tokens with the contract to get voting power in the DAO
    Stake {},
    /// Unstakes tokens so that they begin unbonding
    Unstake {
        amount: Uint128,
    },
    Claim {},
}
#[cw_serde]
pub enum DaoDaoRewardsExecuteMsg {
    Claim {
        id: u64,
    },
}

#[cw_serde]
pub enum DaoDaoProposalSingleExecuteMsg {
    Vote {
        /// The ID of the proposal to vote on.
        proposal_id: u64,
        /// The senders position on the proposal.
        vote: Vote,
        /// An optional rationale for why this vote was cast. This can
        /// be updated, set, or removed later by the address casting
        /// the vote.
        rationale: Option<String>,
    },
}

#[cw_serde]
#[derive(Copy)]
#[repr(u8)]
pub enum Vote {
    /// Marks support for the proposal.
    Yes,
    /// Marks opposition to the proposal.
    No,
    /// Marks participation but does not count towards the ratio of
    /// support / opposed.
    Abstain,
}

#[cw_serde]
pub enum DaoDaoQueryMsg {
    Proposal {
        proposal_id: u64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct DaoDaoProposalResponse {
    /// The ID of the proposal being returned.
    pub id: u64,
    pub proposal: DaoDaoSingleChoiceProposal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct DaoDaoSingleChoiceProposal {
    pub expiration: Expiration,
}

#[cw_serde]
pub enum CwRewardsExecuteMsg {
    ClaimRewards(CwRewardsClaimRewardsMsg),
}
#[cw_serde]
pub struct CwRewardsClaimRewardsMsg {
    pub callback: Option<KujiraCallbackData>,
}

#[cw_serde]
pub struct KujiraCallbackData(pub Binary);

impl DaoInterface<Addr> {
    pub fn deposit_msg(
        &self,
        utoken: &AssetInfo,
        amount: Uint128,
        eris_contract_addr: String,
    ) -> StdResult<CosmosMsg<CustomMsgType>> {
        match utoken {
            AssetInfo::Token {
                contract_addr,
            } => match &self {
                DaoInterface::Enterprise {
                    addr,
                    ..
                } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_json_binary(&Cw20ExecuteMsg::Send {
                        contract: addr.to_string(),
                        amount,
                        msg: to_json_binary(&EnterpriseCw20HookMsg::Stake {})?,
                    })?,
                    funds: vec![],
                })),
                DaoInterface::EnterpriseV2 {
                    membership,
                    ..
                } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_json_binary(&Cw20ExecuteMsg::Send {
                        contract: membership.to_string(),
                        amount,
                        msg: to_json_binary(&EnterpriseV2Cw20HookMsg::Stake {
                            user: eris_contract_addr,
                        })?,
                    })?,
                    funds: vec![],
                })),

                DaoInterface::Cw4 {
                    addr,
                    ..
                } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_json_binary(&Cw20ExecuteMsg::Send {
                        contract: addr.to_string(),
                        amount,
                        msg: to_json_binary(&cw4_stake::msg::ReceiveMsg::Bond {})?,
                    })?,
                    funds: vec![],
                })),
                DaoInterface::Alliance {
                    addr,
                } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_json_binary(&Cw20ExecuteMsg::Send {
                        contract: addr.to_string(),
                        amount,
                        msg: to_json_binary(
                            &alliance_protocol::alliance_protocol::ExecuteMsg::Stake {},
                        )?,
                    })?,
                    funds: vec![],
                })),

                DaoInterface::Capa {
                    gov,
                } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_json_binary(&Cw20ExecuteMsg::Send {
                        contract: gov.to_string(),
                        amount,
                        msg: to_json_binary(&CapaExecuteMsg::StakeVotingTokens {})?,
                    })?,
                    funds: vec![],
                })),
                DaoInterface::DaoDao {
                    ..
                } => Err(StdError::generic_err("cw20 not supported for daodao")),

                DaoInterface::DaoDaoV2 {
                    staking,
                    ..
                } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_json_binary(&Cw20ExecuteMsg::Send {
                        contract: staking.to_string(),
                        amount,
                        msg: to_json_binary(&DaoDaoExecuteMsg::Stake {})?,
                    })?,
                    funds: vec![],
                })),
            },
            AssetInfo::NativeToken {
                denom,
            } => match &self {
                DaoInterface::Enterprise {
                    ..
                }
                | DaoInterface::EnterpriseV2 {
                    ..
                } => Err(StdError::generic_err("native_token not supported for enterprise")),
                DaoInterface::Cw4 {
                    addr,
                    ..
                } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: addr.to_string(),
                    msg: to_json_binary(&cw4_stake::msg::ExecuteMsg::Bond {})?,
                    funds: vec![coin(amount.u128(), denom)],
                })),
                DaoInterface::Alliance {
                    addr,
                } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: addr.to_string(),
                    msg: to_json_binary(
                        &alliance_protocol::alliance_protocol::ExecuteMsg::Stake {},
                    )?,
                    funds: vec![coin(amount.u128(), denom)],
                })),
                DaoInterface::Capa {
                    ..
                } => Err(StdError::generic_err("native_token not supported for capa")),
                DaoInterface::DaoDao {
                    staking,
                    ..
                } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: staking.to_string(),
                    msg: to_json_binary(&DaoDaoExecuteMsg::Stake {})?,
                    funds: vec![coin(amount.u128(), denom)],
                })),
                DaoInterface::DaoDaoV2 {
                    staking,
                    ..
                } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: staking.to_string(),
                    msg: to_json_binary(&DaoDaoExecuteMsg::Stake {})?,
                    funds: vec![coin(amount.u128(), denom)],
                })),
            },
        }
    }

    pub fn unbond_msg(
        &self,
        utoken: &AssetInfo,
        amount: Uint128,
    ) -> StdResult<CosmosMsg<CustomMsgType>> {
        match &self {
            DaoInterface::Enterprise {
                addr,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                msg: to_json_binary(&EnterpriseExecuteMsg::Unstake(EnterpriseUnstakeMsg::Cw20(
                    EnterpriseUnstakeCw20Msg {
                        amount,
                    },
                )))?,
                funds: vec![],
            })),
            DaoInterface::EnterpriseV2 {
                membership,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: membership.to_string(),
                msg: to_json_binary(&EnterpriseExecuteMsgV2::Unstake(EnterpriseUnstakeMsgV2 {
                    amount,
                }))?,
                funds: vec![],
            })),
            DaoInterface::Cw4 {
                addr,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                msg: to_json_binary(&cw4_stake::msg::ExecuteMsg::Unbond {
                    tokens: amount,
                })?,
                funds: vec![],
            })),
            DaoInterface::DaoDao {
                staking,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: staking.to_string(),
                msg: to_json_binary(&DaoDaoExecuteMsg::Unstake {
                    amount,
                })?,
                funds: vec![],
            })),
            DaoInterface::DaoDaoV2 {
                staking,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: staking.to_string(),
                msg: to_json_binary(&DaoDaoExecuteMsg::Unstake {
                    amount,
                })?,
                funds: vec![],
            })),
            DaoInterface::Alliance {
                addr,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                msg: to_json_binary(&alliance_protocol::alliance_protocol::ExecuteMsg::Unstake(
                    to_cw_asset(utoken, amount),
                ))?,
                funds: vec![],
            })),
            DaoInterface::Capa {
                gov,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: gov.to_string(),
                msg: to_json_binary(&CapaExecuteMsg::WithdrawVotingTokens {
                    amount: Some(amount),
                })?,
                funds: vec![],
            })),
        }
    }

    pub fn claim_unbonded_msg(&self) -> StdResult<CosmosMsg<CustomMsgType>> {
        match &self {
            DaoInterface::Enterprise {
                addr,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                msg: to_json_binary(&EnterpriseExecuteMsg::Claim {})?,
                funds: vec![],
            })),
            DaoInterface::EnterpriseV2 {
                membership,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: membership.to_string(),
                msg: to_json_binary(&EnterpriseExecuteMsg::Claim {})?,
                funds: vec![],
            })),
            DaoInterface::Cw4 {
                addr,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                msg: to_json_binary(&cw4_stake::msg::ExecuteMsg::Claim {})?,
                funds: vec![],
            })),
            DaoInterface::DaoDao {
                staking,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: staking.to_string(),
                msg: to_json_binary(&DaoDaoExecuteMsg::Claim {})?,
                funds: vec![],
            })),
            DaoInterface::DaoDaoV2 {
                staking,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: staking.to_string(),
                msg: to_json_binary(&DaoDaoExecuteMsg::Claim {})?,
                funds: vec![],
            })),
            DaoInterface::Alliance {
                ..
            }
            | DaoInterface::Capa {
                ..
            } => Err(StdError::generic_err("claiming not supported for alliance, capa"))?,
        }
    }

    pub fn claim_rewards_msgs(
        &self,
        env: &Env,
        utoken: &AssetInfo,
        native_denoms: Vec<String>,
        cw20_assets: Vec<String>,
    ) -> StdResult<Vec<CosmosMsg<CustomMsgType>>> {
        match &self {
            DaoInterface::Enterprise {
                fund_distributor,
                ..
            } => Ok(vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: fund_distributor.to_string(),
                msg: to_json_binary(&EnterpriseDistributorExecuteMsg::ClaimRewards(
                    EnterpriseClaimRewardsMsg {
                        user: env.contract.address.to_string(),
                        native_denoms: Some(native_denoms),
                        cw20_assets: Some(cw20_assets),
                    },
                ))?,
                funds: vec![],
            })]),
            DaoInterface::EnterpriseV2 {
                distributor,
                ..
            } => Ok(vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: distributor.to_string(),
                msg: to_json_binary(&EnterpriseDistributorExecuteMsg::ClaimRewards(
                    EnterpriseClaimRewardsMsg {
                        user: env.contract.address.to_string(),
                        native_denoms: Some(native_denoms),
                        cw20_assets: Some(cw20_assets),
                    },
                ))?,
                funds: vec![],
            })]),
            DaoInterface::Cw4 {
                fund_distributor,
                ..
            } => Ok(vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: fund_distributor.to_string(),
                msg: to_json_binary(&Cw4DistributorExecuteMsg::ClaimRewards(Cw4ClaimRewardsMsg {
                    user: env.contract.address.to_string(),
                    native_denoms: None,
                }))?,
                funds: vec![],
            })]),
            DaoInterface::DaoDao {
                cw_rewards,
                ..
            } => Ok(vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cw_rewards.to_string(),
                msg: to_json_binary(&CwRewardsExecuteMsg::ClaimRewards(
                    CwRewardsClaimRewardsMsg {
                        callback: None,
                    },
                ))?,
                funds: vec![],
            })]),
            DaoInterface::DaoDaoV2 {
                rewards: cw_rewards,
                ..
            } => Ok(cw_rewards
                .iter()
                .map(|(contract, claim_id)| {
                    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: contract.to_string(),
                        msg: to_json_binary(&DaoDaoRewardsExecuteMsg::Claim {
                            id: *claim_id,
                        })?,
                        funds: vec![],
                    }))
                })
                .collect::<StdResult<Vec<_>>>()?),
            DaoInterface::Alliance {
                addr,
            } => Ok(vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                msg: to_json_binary(
                    &alliance_protocol::alliance_protocol::ExecuteMsg::ClaimRewards(
                        to_cw_asset_info(utoken),
                    ),
                )?,
                funds: vec![],
            })]),
            DaoInterface::Capa {
                gov,
            } => Ok(vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: gov.to_string(),
                msg: to_json_binary(&CapaExecuteMsg::Claim {})?,
                funds: vec![],
            })]),
        }
    }

    pub fn vote_msg(
        &self,
        proposal_id: u64,
        outcome: VoteOption,
        _voter: Addr,
        _amount: Uint128,
    ) -> StdResult<CosmosMsg<CustomMsgType>> {
        match &self {
            DaoInterface::Enterprise {
                addr,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                msg: to_json_binary(&EnterpriseExecuteMsg::CastVote(CastVoteMsg {
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
            DaoInterface::EnterpriseV2 {
                gov,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: gov.to_string(),
                msg: to_json_binary(&EnterpriseExecuteMsg::CastVote(CastVoteMsg {
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
            DaoInterface::Cw4 {
                gov,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: gov.to_string(),
                msg: to_json_binary(&cw3::Cw3ExecuteMsg::Vote::<Empty> {
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
            DaoInterface::DaoDao {
                gov,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: gov.to_string(),
                msg: to_json_binary(&DaoDaoProposalSingleExecuteMsg::Vote {
                    rationale: None,
                    proposal_id,
                    vote: match outcome {
                        VoteOption::Yes => Vote::Yes,
                        VoteOption::No => Vote::No,
                        VoteOption::Abstain => Vote::Abstain,
                        VoteOption::NoWithVeto => Vote::No,
                    },
                })?,
                funds: vec![],
            })),
            DaoInterface::DaoDaoV2 {
                gov,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: gov.to_string(),
                msg: to_json_binary(&DaoDaoProposalSingleExecuteMsg::Vote {
                    rationale: None,
                    proposal_id,
                    vote: match outcome {
                        VoteOption::Yes => Vote::Yes,
                        VoteOption::No => Vote::No,
                        VoteOption::Abstain => Vote::Abstain,
                        VoteOption::NoWithVeto => Vote::No,
                    },
                })?,
                funds: vec![],
            })),
            DaoInterface::Alliance {
                ..
            }
            | DaoInterface::Capa {
                ..
            } => Err(StdError::generic_err("voting not supported for alliance, capa"))?,
            // DaoInterface::Capa {
            //     ..
            // } => Err(StdError::generic_err("voting not supported for capa"))?,
            // Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            //     contract_addr: gov.to_string(),
            //     msg: to_json_binary(&CapaExecuteMsg::CastVote {
            //         poll_id: proposal_id,
            //         vote: match outcome {
            //             VoteOption::Yes => CapaVoteOption::Yes,
            //             VoteOption::No => CapaVoteOption::No,
            //             VoteOption::Abstain => {
            //                 Err(StdError::generic_err("voting abstain not supported for capa"))?
            //             },
            //             VoteOption::NoWithVeto => CapaVoteOption::No,
            //         },
            //         amount: ,
            //     })?,
            //     funds: vec![],
            // })),
        }
    }

    pub fn query_proposal(
        &self,
        querier: &QuerierWrapper,
        proposal_id: u64,
    ) -> StdResult<ProposalResponse> {
        match self {
            DaoInterface::Enterprise {
                addr,
                ..
            } => {
                let result: EnterpriseProposalResponse = querier.query_wasm_smart(
                    addr,
                    &EnterpriseQueryMsg::Proposal(ProposalParams {
                        proposal_id,
                    }),
                )?;

                Ok(ProposalResponse {
                    end_time_s: match result.proposal.expires {
                        Expiration::AtHeight(_) => {
                            Err(StdError::generic_err("not supported expiry type."))
                        },
                        Expiration::AtTime(time) => Ok(time.seconds()),
                        Expiration::Never {} => {
                            Err(StdError::generic_err("not supported expiry type."))
                        },
                    }?,
                })
            },
            DaoInterface::EnterpriseV2 {
                gov,
                ..
            } => {
                let result: EnterpriseProposalResponse = querier.query_wasm_smart(
                    gov,
                    &EnterpriseQueryMsg::Proposal(ProposalParams {
                        proposal_id,
                    }),
                )?;

                Ok(ProposalResponse {
                    end_time_s: match result.proposal.expires {
                        Expiration::AtHeight(_) => {
                            Err(StdError::generic_err("not supported expiry type."))
                        },
                        Expiration::AtTime(time) => Ok(time.seconds()),
                        Expiration::Never {} => {
                            Err(StdError::generic_err("not supported expiry type."))
                        },
                    }?,
                })
            },
            DaoInterface::Cw4 {
                gov,
                ..
            } => {
                let result: Cw3ProposalResponse = querier.query_wasm_smart(
                    gov,
                    &Cw3QueryMsg::Proposal {
                        proposal_id,
                    },
                )?;

                Ok(ProposalResponse {
                    end_time_s: match result.expires {
                        Expiration::AtHeight(_) => {
                            Err(StdError::generic_err("not supported expiry type."))
                        },
                        Expiration::AtTime(time) => Ok(time.seconds()),
                        Expiration::Never {} => {
                            Err(StdError::generic_err("not supported expiry type."))
                        },
                    }?,
                })
            },
            DaoInterface::DaoDao {
                gov,
                ..
            } => {
                let result: DaoDaoProposalResponse = querier.query_wasm_smart(
                    gov,
                    &DaoDaoQueryMsg::Proposal {
                        proposal_id,
                    },
                )?;

                Ok(ProposalResponse {
                    end_time_s: match result.proposal.expiration {
                        Expiration::AtHeight(_) => {
                            Err(StdError::generic_err("not supported expiry type."))
                        },
                        Expiration::AtTime(time) => Ok(time.seconds()),
                        Expiration::Never {} => {
                            Err(StdError::generic_err("not supported expiry type."))
                        },
                    }?,
                })
            },
            DaoInterface::DaoDaoV2 {
                gov,
                ..
            } => {
                let result: DaoDaoProposalResponse = querier.query_wasm_smart(
                    gov,
                    &DaoDaoQueryMsg::Proposal {
                        proposal_id,
                    },
                )?;

                Ok(ProposalResponse {
                    end_time_s: match result.proposal.expiration {
                        Expiration::AtHeight(_) => {
                            Err(StdError::generic_err("not supported expiry type."))
                        },
                        Expiration::AtTime(time) => Ok(time.seconds()),
                        Expiration::Never {} => {
                            Err(StdError::generic_err("not supported expiry type."))
                        },
                    }?,
                })
            },
            DaoInterface::Alliance {
                ..
            }
            | DaoInterface::Capa {
                ..
            } => Err(StdError::generic_err("proposal not supported for alliance, capa"))?,
        }
    }
}

fn to_cw_asset(utoken: &AssetInfo, amount: Uint128) -> cw_asset::Asset {
    match utoken {
        AssetInfo::Token {
            contract_addr,
        } => cw_asset::Asset::cw20(contract_addr.clone(), amount),
        AssetInfo::NativeToken {
            denom,
        } => cw_asset::Asset::native(denom, amount),
    }
}

fn to_cw_asset_info(utoken: &AssetInfo) -> cw_asset::AssetInfo {
    match utoken {
        AssetInfo::Token {
            contract_addr,
        } => cw_asset::AssetInfo::cw20(contract_addr.clone()),
        AssetInfo::NativeToken {
            denom,
        } => cw_asset::AssetInfo::native(denom),
    }
}
