use astroport::asset::{token_asset_info, AssetInfo};
use cosmwasm_std::{
    entry_point, from_json, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult,
};
use cw2::set_contract_version;

use cw20::Cw20ReceiveMsg;
use eris::helper::validate_received_funds;
use eris::hub_alliance::{
    CallbackMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, ReceiveMsg,
};
use eris_chain_adapter::types::CustomQueryType;

use crate::constants::{CONTRACT_NAME, CONTRACT_VERSION};
use crate::error::{ContractError, ContractResult};
use crate::state::State;
use crate::{execute, queries};

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
                api.addr_validate(&receiver.unwrap_or_else(|| info.sender.to_string()))?,
                donate.unwrap_or(false),
            )
        },

        ExecuteMsg::Unbond {
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

            execute::unbond(
                deps,
                env,
                info.sender.clone(),
                api.addr_validate(&receiver.unwrap_or_else(|| info.sender.to_string()))?,
                info.funds[0].amount,
            )
        },

        ExecuteMsg::Swap {
            offer_asset,
            to,
            ..
        } => {
            offer_asset.assert_sent_native_token_balance(&info)?;

            let state = State::default();
            let stake = state.stake_token.load(deps.storage)?;

            if stake.utoken == offer_asset.info {
                let token_to_bond = offer_asset.amount;
                return execute::bond(
                    deps,
                    env,
                    state,
                    stake,
                    token_to_bond,
                    api.addr_validate(&to.unwrap_or_else(|| info.sender.to_string()))?,
                    false,
                );
            } else if let AssetInfo::NativeToken {
                denom,
            } = offer_asset.info
            {
                if denom == stake.denom {
                    return execute::unbond(
                        deps,
                        env,
                        info.sender.clone(),
                        api.addr_validate(&to.unwrap_or_else(|| info.sender.to_string()))?,
                        offer_asset.amount,
                    );
                }
            }

            Err(ContractError::ExpectingSupportedTokens {})
        },

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
        ExecuteMsg::Callback(callback_msg) => callback(deps, env, info, callback_msg),

        ExecuteMsg::UpdateConfig {
            protocol_fee_contract,
            protocol_reward_fee,
            operator,
            stages_preset,
            allow_donations,
            withdrawals_preset,
            default_max_spread,
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
            default_max_spread,
        ),
    }
}

fn receive(deps: DepsMut, env: Env, info: MessageInfo, cw20_msg: Cw20ReceiveMsg) -> ContractResult {
    let api = deps.api;
    match from_json(&cw20_msg.msg)? {
        ReceiveMsg::Swap {
            to,
            ..
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
                api.addr_validate(&to.unwrap_or(cw20_msg.sender))?,
                false,
            )
        },
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
        CallbackMsg::Reinvest {
            skip_fee,
        } => execute::reinvest(deps, env, skip_fee),
        CallbackMsg::WithdrawLps {
            withdrawals,
        } => execute::withdraw_lps(deps, env, withdrawals),
        CallbackMsg::SingleStageSwap {
            stage,
            index,
        } => execute::single_stage_swap(deps, env, stage, index),
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
        QueryMsg::ExchangeRates {
            start_after,
            limit,
        } => to_json_binary(&queries::query_exchange_rates(deps, env, start_after, limit)?),
        QueryMsg::Pair {} => to_json_binary(&queries::query_pair(deps, env)?),
    }
}

#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> ContractResult {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new()
        .add_attribute("new_contract_name", CONTRACT_NAME)
        .add_attribute("new_contract_version", CONTRACT_VERSION))
}
