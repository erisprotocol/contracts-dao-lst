use astroport::asset::token_asset_info;
use cosmwasm_std::{
    attr, entry_point, from_json, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order,
    Response, StdResult,
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
                stake_token,
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
    match from_json(&cw20_msg.msg)? {
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
        QueryMsg::Config {} => to_json_binary(&queries::config(deps)?),
        QueryMsg::State {} => to_json_binary(&queries::state(deps, env)?),
        QueryMsg::PendingBatch {} => to_json_binary(&queries::pending_batch(deps)?),
        QueryMsg::PreviousBatch(id) => to_json_binary(&queries::previous_batch(deps, id)?),
        QueryMsg::PreviousBatches {
            start_after,
            limit,
        } => to_json_binary(&queries::previous_batches(deps, start_after, limit)?),
        QueryMsg::UnbondRequestsByBatch {
            id,
            start_after,
            limit,
        } => to_json_binary(&queries::unbond_requests_by_batch(deps, id, start_after, limit)?),
        QueryMsg::UnbondRequestsByUser {
            user,
            start_after,
            limit,
        } => to_json_binary(&queries::unbond_requests_by_user(deps, user, start_after, limit)?),

        QueryMsg::UnbondRequestsByUserDetails {
            user,
            start_after,
            limit,
        } => to_json_binary(&queries::unbond_requests_by_user_details(
            deps,
            user,
            start_after,
            limit,
            env,
        )?),

        QueryMsg::ExchangeRates {
            start_after,
            limit,
        } => to_json_binary(&queries::query_exchange_rates(deps, env, start_after, limit)?),
    }
}

#[entry_point]
pub fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg) -> ContractResult {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let mut msgs = vec![];
    let mut attrs = vec![];
    if let Some(action) = msg.action {
        let state = State::default();
        let mut stake_token = state.stake_token.load(deps.storage)?;
        match action {
            eris::hub::MigrateAction::AllowSubmit => {

            let mut pending = state.pending_batch.load(deps.storage)?;
            pending.est_unbond_start_time = env.block.time.seconds() ;
            state.pending_batch.save(deps.storage, &pending)?;
        },
            eris::hub::MigrateAction::Disable => {
                stake_token.disabled = true;
                state.stake_token.save(deps.storage, &stake_token)?;
                attrs.push(attr("action", "disable"));
                attrs.push(attr("disabled", "true"));
            },
            eris::hub::MigrateAction::Unstake => {
                let unstake_msg = stake_token
                    .dao_interface
                    .unbond_msg(&stake_token.utoken, stake_token.total_utoken_bonded)?;
                msgs.push(unstake_msg);
                attrs.push(attr("action", "unstake"));
                attrs.push(attr("unstake", stake_token.total_utoken_bonded));
            },
            eris::hub::MigrateAction::Claim => {
                let claim_msg = stake_token.dao_interface.claim_unbonded_msg()?;
                msgs.push(claim_msg);
                attrs.push(attr("action", "claim"));
            },
            eris::hub::MigrateAction::ReconcileAll => {
                // Load batches that have not been reconciled
                let all_batches = state
                    .previous_batches
                    .idx
                    .reconciled
                    .prefix(false.into())
                    .range(deps.storage, None, None, Order::Ascending)
                    .map(|item| {
                        let (_, v) = item?;
                        Ok(v)
                    })
                    .collect::<StdResult<Vec<_>>>()?;

                let mut ids: Vec<String> = vec![];
                for mut batch in all_batches {
                    batch.reconciled = true;
                    if batch.est_unbond_end_time > env.block.time.seconds() {
                        batch.est_unbond_end_time = env.block.time.seconds();
                    }
                    ids.push(batch.id.to_string());
                    state.previous_batches.save(deps.storage, batch.id, &batch)?;
                }

                attrs.push(attr("action", "reconcile"));
                attrs.push(attr("ids", ids.join(",")));
            },
            eris::hub::MigrateAction::Stake => {
                let stake_msg = stake_token.dao_interface.deposit_msg(
                    &stake_token.utoken,
                    stake_token.total_utoken_bonded,
                    env.contract.address.to_string(),
                )?;

                msgs.push(stake_msg);
                attrs.push(attr("action", "stake"));
                attrs.push(attr("stake", stake_token.total_utoken_bonded));
            },
            eris::hub::MigrateAction::Enable => {
                let mut stake_token = state.stake_token.load(deps.storage)?;
                stake_token.disabled = false;
                state.stake_token.save(deps.storage, &stake_token)?;
                attrs.push(attr("action", "enable"));
                attrs.push(attr("disabled", "false"));
            },
        }
    }

    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("new_contract_name", CONTRACT_NAME)
        .add_attribute("new_contract_version", CONTRACT_VERSION)
        .add_attributes(attrs))
}
