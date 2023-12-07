use astroport::asset::token_asset_info;
use cosmwasm_std::{
    entry_point, from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult,
};
use cw2::set_contract_version;

use cw20::Cw20ReceiveMsg;
use eris::helper::validate_received_funds;
use eris::hub::{CallbackMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, ReceiveMsg};
use eris_chain_adapter::types::CustomQueryType;

use crate::claim::exec_claim;
use crate::constants::{CONTRACT_NAME, CONTRACT_VERSION};
use crate::error::{ContractError, ContractResult};
use crate::state::State;
use crate::{execute, gov, queries};

#[entry_point]
pub fn instantiate(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult {
    execute::instantiate(deps, env, msg)
}

#[entry_point]
pub fn execute(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult {
    let api = deps.api;
    match msg {
        ExecuteMsg::Receive(cw20_msg) => receive(deps, env, info, cw20_msg),
        ExecuteMsg::Bond {
            receiver,
            donate,
        } => {
            let state = State::default();
            let stake = state.stake_token.load(deps.storage)?;
            let token_to_bond = validate_received_funds(&info.funds, &stake.utoken)?;

            execute::bond(
                deps,
                env,
                state,
                stake,
                token_to_bond,
                receiver.map(|s| api.addr_validate(&s)).transpose()?.unwrap_or(info.sender),
                donate.unwrap_or(false),
            )
        },
        ExecuteMsg::WithdrawUnbonded {
            receiver,
        } => execute::withdraw_unbonded(
            deps,
            env,
            info.sender.clone(),
            receiver.map(|s| api.addr_validate(&s)).transpose()?.unwrap_or(info.sender),
        ),
        ExecuteMsg::TransferOwnership {
            new_owner,
        } => execute::transfer_ownership(deps, info.sender, new_owner),
        ExecuteMsg::DropOwnershipProposal {} => execute::drop_ownership_proposal(deps, info.sender),
        ExecuteMsg::AcceptOwnership {} => execute::accept_ownership(deps, info.sender),
        ExecuteMsg::Harvest {
            withdrawals,
            stages,
            native_denoms,
            cw20_assets,
            router,
        } => execute::harvest(
            deps,
            env,
            native_denoms,
            cw20_assets,
            withdrawals,
            stages,
            router,
            info.sender,
        ),
        ExecuteMsg::Reconcile {} => execute::reconcile(deps, env),
        ExecuteMsg::SubmitBatch {} => execute::submit_batch(deps, env),
        ExecuteMsg::Vote {
            proposal_id,
            vote,
        } => gov::vote(deps, env, info, proposal_id, vote),

        ExecuteMsg::Callback(callback_msg) => callback(deps, env, info, callback_msg),

        ExecuteMsg::UpdateConfig {
            protocol_fee_contract,
            protocol_reward_fee,
            operator,
            stages_preset,
            allow_donations,
            vote_operator,

            withdrawals_preset,
            default_max_spread,
            epoch_period,
            unbond_period,
            dao_interface,
        } => execute::update_config(
            env,
            deps,
            info.sender,
            protocol_fee_contract,
            protocol_reward_fee,
            operator,
            stages_preset,
            withdrawals_preset,
            allow_donations,
            vote_operator,
            default_max_spread,
            epoch_period,
            unbond_period,
            dao_interface,
        ),
        ExecuteMsg::QueueUnbond {
            receiver,
        } => {
            let state = State::default();
            let stake_token = state.stake_token.load(deps.storage)?;

            if info.funds.len() != 1 {
                return Err(ContractError::ExpectingSingleCoin {});
            }

            if info.funds[0].denom != stake_token.denom {
                return Err(ContractError::ExpectingStakeToken(info.funds[0].denom.to_string()));
            }

            execute::queue_unbond(
                deps,
                env,
                api.addr_validate(&receiver.unwrap_or_else(|| info.sender.to_string()))?,
                info.funds[0].amount,
            )
        },
        ExecuteMsg::Claim {
            claims,
        } => exec_claim(deps, env, info, claims),
    }
}

fn receive(deps: DepsMut, env: Env, info: MessageInfo, cw20_msg: Cw20ReceiveMsg) -> ContractResult {
    let api = deps.api;
    match from_binary(&cw20_msg.msg)? {
        ReceiveMsg::Bond {
            receiver,
            donate,
        } => {
            let state = State::default();
            let stake_token = state.stake_token.load(deps.storage)?;

            if token_asset_info(info.sender.clone()) != stake_token.utoken {
                return Err(ContractError::ExpectingStakeToken(info.sender.into()));
            }

            execute::bond(
                deps,
                env,
                state,
                stake_token,
                cw20_msg.amount,
                api.addr_validate(&receiver.unwrap_or(cw20_msg.sender))?,
                donate.unwrap_or(false),
            )
        },
    }
}

fn callback(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    info: MessageInfo,
    callback_msg: CallbackMsg,
) -> ContractResult {
    if env.contract.address != info.sender {
        return Err(ContractError::CallbackOnlyCalledByContract {});
    }

    match callback_msg {
        CallbackMsg::Reinvest {} => execute::reinvest(deps, env),
        CallbackMsg::WithdrawLps {
            withdrawals,
        } => execute::withdraw_lps(deps, env, withdrawals),
        CallbackMsg::SingleStageSwap {
            stage,
        } => execute::single_stage_swap(deps, env, stage),
        CallbackMsg::MultiSwapRouter {
            router,
        } => execute::multi_swap_router(deps, env, router),
        CallbackMsg::CheckReceivedCoin {
            snapshot,
            snapshot_stake,
        } => execute::callback_received_coins(deps, env, snapshot, snapshot_stake),
        CallbackMsg::AssertBalance {
            expected,
        } => execute::callback_assert_balance(deps, env, expected),
    }
}

#[entry_point]
pub fn query(deps: Deps<CustomQueryType>, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&queries::config(deps)?),
        QueryMsg::State {} => to_binary(&queries::state(deps, env)?),
        QueryMsg::PendingBatch {} => to_binary(&queries::pending_batch(deps)?),
        QueryMsg::PreviousBatch(id) => to_binary(&queries::previous_batch(deps, id)?),
        QueryMsg::PreviousBatches {
            start_after,
            limit,
        } => to_binary(&queries::previous_batches(deps, start_after, limit)?),
        QueryMsg::UnbondRequestsByBatch {
            id,
            start_after,
            limit,
        } => to_binary(&queries::unbond_requests_by_batch(deps, id, start_after, limit)?),
        QueryMsg::UnbondRequestsByUser {
            user,
            start_after,
            limit,
        } => to_binary(&queries::unbond_requests_by_user(deps, user, start_after, limit)?),

        QueryMsg::UnbondRequestsByUserDetails {
            user,
            start_after,
            limit,
        } => to_binary(&queries::unbond_requests_by_user_details(
            deps,
            user,
            start_after,
            limit,
            env,
        )?),

        QueryMsg::ExchangeRates {
            start_after,
            limit,
        } => to_binary(&queries::query_exchange_rates(deps, env, start_after, limit)?),
    }
}

#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> ContractResult {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new()
        .add_attribute("new_contract_name", CONTRACT_NAME)
        .add_attribute("new_contract_version", CONTRACT_VERSION))
}
