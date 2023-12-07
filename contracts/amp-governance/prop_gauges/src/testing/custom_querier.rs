use std::collections::HashMap;
use std::ops::Mul;

use astroport::asset::native_asset_info;
use cosmwasm_schema::serde::Serialize;
use cosmwasm_std::testing::{BankQuerier, StakingQuerier, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, from_slice, to_binary, Addr, Coin, Decimal, Empty, Querier, QuerierResult,
    QueryRequest, SystemError, Timestamp, Uint128, WasmQuery,
};
use cw20::Cw20QueryMsg;
use eris::adapters::dao::{
    Cw3ProposalResponse, EnterprisePollResponse, EnterpriseProposalResponse,
};
use eris::voting_escrow::{LockInfoResponse, VotingPowerResponse};

use super::cw20_querier::Cw20Querier;
use super::helpers::err_unsupported_query;

#[derive(Default)]
pub(super) struct CustomQuerier {
    pub cw20_querier: Cw20Querier,
    pub bank_querier: BankQuerier,
    pub staking_querier: StakingQuerier,

    pub vp: HashMap<String, LockInfoResponse>,
    pub prop_map: HashMap<u64, u64>,
}

impl Querier for CustomQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let request: QueryRequest<_> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
                .into()
            },
        };
        self.handle_query(&request)
    }
}

impl CustomQuerier {
    #[allow(dead_code)]
    pub fn set_cw20_balance(&mut self, token: &str, user: &str, balance: u128) {
        match self.cw20_querier.balances.get_mut(token) {
            Some(contract_balances) => {
                contract_balances.insert(user.to_string(), balance);
            },
            None => {
                let mut contract_balances: HashMap<String, u128> = HashMap::default();
                contract_balances.insert(user.to_string(), balance);
                self.cw20_querier.balances.insert(token.to_string(), contract_balances);
            },
        };
    }

    #[allow(dead_code)]
    pub fn set_cw20_total_supply(&mut self, token: &str, total_supply: u128) {
        self.cw20_querier.total_supplies.insert(token.to_string(), total_supply);
    }

    #[allow(dead_code)]
    pub fn set_bank_balances(&mut self, balances: &[Coin]) {
        self.bank_querier = BankQuerier::new(&[(MOCK_CONTRACT_ADDR, balances)])
    }

    pub fn set_lock(&mut self, user: impl Into<String>, fixed: u128, dynamic: u128) {
        self.vp.insert(
            user.into(),
            LockInfoResponse {
                amount: Uint128::zero(),
                coefficient: Decimal::zero(),
                start: 0,
                end: 10,
                slope: Uint128::new(1),
                fixed_amount: Uint128::new(fixed),
                voting_power: Uint128::new(dynamic),
            },
        );
    }

    pub fn set_prop_expiry(&mut self, proposal: u64, end_time_s: u64) {
        self.prop_map.insert(proposal, end_time_s);
    }

    pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match request {
            QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr,
                msg,
            }) => {
                if let Ok(query) = from_binary::<Cw20QueryMsg>(msg) {
                    return self.cw20_querier.handle_query(contract_addr, query);
                }

                if let Ok(eris::hub::QueryMsg::Config {}) = from_binary::<eris::hub::QueryMsg>(msg)
                {
                    return self.to_result(eris::hub::ConfigResponse {
                        owner: "owner".to_string(),
                        new_owner: None,
                        stake_token: "factory/abc/ampXXX".to_string(),
                        epoch_period: 259200,
                        unbond_period: 1814400,
                        fee_config: eris::hub::FeeConfig {
                            protocol_fee_contract: Addr::unchecked("fee"),
                            protocol_reward_fee: Decimal::from_ratio(1u128, 100u128),
                        },
                        operator: "operator".to_string(),
                        stages_preset: vec![],
                        withdrawals_preset: vec![],
                        allow_donations: false,
                        vote_operator: None,
                        utoken: native_asset_info("utoken".to_string()),
                        dao_interface: eris::hub::DaoInterface::Cw4 {
                            addr: Addr::unchecked("cw4"),
                            gov: Addr::unchecked("gov"),
                            fund_distributor: Addr::unchecked("fund"),
                        },
                    });
                }

                if let Ok(query) = from_binary::<eris::voting_escrow::QueryMsg>(msg) {
                    return self.handle_vp_query(contract_addr, query);
                }

                if let Ok(query) = from_binary::<eris::adapters::dao::Cw3QueryMsg>(msg) {
                    return match query {
                        eris::adapters::dao::Cw3QueryMsg::Proposal {
                            proposal_id,
                        } => match self.prop_map.get(&proposal_id) {
                            Some(val) => self.to_result(Cw3ProposalResponse {
                                id: proposal_id,
                                expires: cw20::Expiration::AtTime(Timestamp::from_seconds(*val)),
                            }),
                            None => err_unsupported_query(msg),
                        },
                    };
                }

                if let Ok(query) = from_binary::<eris::adapters::dao::EnterpriseQueryMsg>(msg) {
                    return match query {
                        eris::adapters::dao::EnterpriseQueryMsg::Proposal(params) => {
                            match self.prop_map.get(&params.proposal_id) {
                                Some(val) => self.to_result(EnterpriseProposalResponse {
                                    proposal: eris::adapters::dao::Proposal {
                                        id: params.proposal_id,
                                        expires: cw20::Expiration::AtTime(Timestamp::from_seconds(
                                            *val,
                                        )),
                                    },
                                }),
                                None => err_unsupported_query(msg),
                            }
                        },
                        eris::adapters::dao::EnterpriseQueryMsg::Poll(params) => {
                            match self.prop_map.get(&params.poll_id) {
                                Some(val) => self.to_result(EnterprisePollResponse {
                                    poll: eris::adapters::dao::Poll {
                                        id: params.poll_id,
                                        ends_at: Timestamp::from_seconds(*val),
                                    },
                                }),
                                None => err_unsupported_query(msg),
                            }
                        },
                    };
                }

                err_unsupported_query(msg)
            },

            QueryRequest::Bank(query) => self.bank_querier.query(query),

            QueryRequest::Staking(query) => self.staking_querier.query(query),

            _ => err_unsupported_query(request),
        }
    }

    pub fn to_result<T>(&self, val: T) -> QuerierResult
    where
        T: Serialize + Sized,
    {
        Ok(to_binary(&val).into()).into()
    }

    fn handle_vp_query(
        &self,
        _contract_addr: &str,
        query: eris::voting_escrow::QueryMsg,
    ) -> QuerierResult {
        match query {
            eris::voting_escrow::QueryMsg::CheckVotersAreBlacklisted {
                ..
            } => todo!(),
            eris::voting_escrow::QueryMsg::BlacklistedVoters {
                ..
            } => todo!(),
            eris::voting_escrow::QueryMsg::Balance {
                ..
            } => todo!(),
            eris::voting_escrow::QueryMsg::TokenInfo {} => todo!(),
            eris::voting_escrow::QueryMsg::MarketingInfo {} => todo!(),
            eris::voting_escrow::QueryMsg::DownloadLogo {} => todo!(),
            eris::voting_escrow::QueryMsg::TotalVamp {} => todo!(),
            eris::voting_escrow::QueryMsg::TotalVampAt {
                ..
            } => todo!(),
            eris::voting_escrow::QueryMsg::TotalVampAtPeriod {
                period,
            } => {
                let mut vamp = Uint128::zero();

                for x in self.vp.values() {
                    if period >= x.start {
                        let diff = period - x.start;
                        vamp = vamp + x.fixed_amount + x.voting_power
                            - x.slope.mul(Uint128::new(diff.into()));
                    }
                }

                self.to_result(VotingPowerResponse {
                    vamp,
                })
            },
            eris::voting_escrow::QueryMsg::UserVamp {
                ..
            } => todo!(),
            eris::voting_escrow::QueryMsg::UserVampAt {
                ..
            } => todo!(),
            eris::voting_escrow::QueryMsg::UserVampAtPeriod {
                ..
            } => todo!(),
            eris::voting_escrow::QueryMsg::LockInfo {
                user,
            } => self.to_result(self.vp.get(&user)),
            eris::voting_escrow::QueryMsg::UserDepositAtHeight {
                ..
            } => todo!(),
            eris::voting_escrow::QueryMsg::Config {} => todo!(),
        }
    }
}
