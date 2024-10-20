use std::cmp;

use astroport::asset::{native_asset, native_asset_info, Asset, AssetInfoExt};
use cosmwasm_std::{
    attr, to_json_binary, Addr, Attribute, CosmosMsg, Decimal, DepsMut, Env, Event, Order,
    Response, StdResult, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use eris::adapters::asset::AssetEx;
use eris::{CustomEvent, CustomResponse, DecimalCheckedOps};

use eris::hub::{
    Batch, CallbackMsg, DaoInterface, ExecuteMsg, FeeConfig, InstantiateMsg, MultiSwapRouter,
    PendingBatch, SingleSwapConfig, StakeToken, UnbondRequest,
};
use eris_chain_adapter::types::{
    chain, get_balances_hashmap, CoinType, CustomMsgType, CustomQueryType, DenomType, WithdrawType,
};
use itertools::Itertools;

use crate::constants::get_reward_fee_cap;
use crate::error::{ContractError, ContractResult};

use crate::math::{compute_mint_amount, compute_unbond_amount};
use crate::state::State;
use crate::types::Assets;

use eris_chain_shared::chain_trait::ChainInterface;

const CONTRACT_NAME: &str = "eris-dao-lst";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

//--------------------------------------------------------------------------------------------------
// Instantiation
//--------------------------------------------------------------------------------------------------

pub fn instantiate(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    msg: InstantiateMsg,
) -> ContractResult {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let state = State::default();
    let chain = chain(&env);

    if msg.protocol_reward_fee.gt(&get_reward_fee_cap()) {
        return Err(ContractError::ProtocolRewardFeeTooHigh {});
    }

    if msg.epoch_period == 0 {
        return Err(ContractError::CantBeZero("epoch_period".into()));
    }

    if msg.unbond_period == 0 {
        return Err(ContractError::CantBeZero("unbond_period".into()));
    }

    state.owner.save(deps.storage, &deps.api.addr_validate(&msg.owner)?)?;
    state.operator.save(deps.storage, &deps.api.addr_validate(&msg.operator)?)?;
    state.epoch_period.save(deps.storage, &msg.epoch_period)?;
    state.unbond_period.save(deps.storage, &msg.unbond_period)?;

    if let Some(vote_operator) = msg.vote_operator {
        state.vote_operator.save(deps.storage, &deps.api.addr_validate(&vote_operator)?)?;
    }

    // by default donations are set to false
    state.allow_donations.save(deps.storage, &false)?;

    state.fee_config.save(
        deps.storage,
        &FeeConfig {
            protocol_fee_contract: deps.api.addr_validate(&msg.protocol_fee_contract)?,
            protocol_reward_fee: msg.protocol_reward_fee,
        },
    )?;

    state.pending_batch.save(
        deps.storage,
        &PendingBatch {
            id: 1,
            ustake_to_burn: Uint128::zero(),
            est_unbond_start_time: env.block.time.seconds() + msg.epoch_period,
        },
    )?;

    let sub_denom = msg.denom;
    let full_denom = chain.get_token_denom(env.contract.address, sub_denom.clone());

    state.unlocked_coins.save(deps.storage, &vec![])?;
    state.stake_token.save(
        deps.storage,
        &StakeToken {
            dao_interface: msg.dao_interface.validate(deps.api)?,
            utoken: msg.utoken,
            denom: full_denom.clone(),
            total_utoken_bonded: Uint128::zero(),
            total_supply: Uint128::zero(),
            disabled: false,
        },
    )?;

    Ok(Response::new().add_message(chain.create_denom_msg(full_denom, sub_denom)))
}

//--------------------------------------------------------------------------------------------------
// Bonding and harvesting logics
//--------------------------------------------------------------------------------------------------

/// NOTE: In a previous implementation, we split up the deposited Token over all validators, so that
/// they all have the same amount of delegation. This is however quite gas-expensive: $1.5 cost in
/// the case of 15 validators.
///
/// To save gas for users, now we simply delegate all deposited Token to the validator with the
/// smallest amount of delegation. If delegations become severely unbalance as a result of this
/// (e.g. when a single user makes a very big deposit), anyone can invoke `ExecuteMsg::Rebalance`
/// to balance the delegations.
pub fn bond(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    state: State,
    mut stake: StakeToken,
    token_to_bond: Uint128,

    receiver: Addr,
    donate: bool,
) -> ContractResult {
    assert_not_disabled(&stake)?;
    // Query the current supply of Staking Token and compute the amount to mint
    let ustake_supply = stake.total_supply;
    let ustake_to_mint = if donate {
        match state.allow_donations.may_load(deps.storage)? {
            Some(false) => Err(ContractError::DonationsDisabled {})?,
            Some(true) | None => {
                // if it is not set (backward compatibility) or set to true, donations are allowed
            },
        }
        Uint128::zero()
    } else {
        compute_mint_amount(ustake_supply, token_to_bond, stake.total_utoken_bonded)
    };

    let event = Event::new("erishub/bonded")
        .add_attribute("receiver", receiver.clone())
        .add_attribute("token_bonded", token_to_bond)
        .add_attribute("ustake_minted", ustake_to_mint);

    let mint_msgs: Option<Vec<CosmosMsg<CustomMsgType>>> = if donate {
        None
    } else {
        // create mint message and add to stored total supply
        stake.total_supply = stake.total_supply.checked_add(ustake_to_mint)?;

        Some(chain(&env).create_mint_msgs(stake.denom.clone(), ustake_to_mint, receiver))
    };
    stake.total_utoken_bonded = stake.total_utoken_bonded.checked_add(token_to_bond)?;
    state.stake_token.save(deps.storage, &stake)?;

    Ok(Response::new()
        .add_message(stake.dao_interface.deposit_msg(
            &stake.utoken,
            token_to_bond,
            env.contract.address.to_string(),
        )?)
        .add_optional_messages(mint_msgs)
        .add_event(event)
        .add_attribute("action", "erishub/bond"))
}

fn assert_not_disabled(stake: &StakeToken) -> Result<(), ContractError> {
    if stake.disabled {
        return Err(ContractError::DisabledMaintenance {});
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn harvest(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    native_denoms: Option<Vec<String>>,
    cw20_assets: Option<Vec<String>>,
    withdrawals: Option<Vec<(WithdrawType, DenomType)>>,
    stages: Option<Vec<Vec<SingleSwapConfig>>>,
    router: Option<MultiSwapRouter>,
    sender: Addr,
) -> ContractResult {
    let state = State::default();
    let stake = state.stake_token.load(deps.storage)?;
    assert_not_disabled(&stake)?;

    // 1. Withdraw rewards
    let claim_msgs = stake.dao_interface.claim_rewards_msgs(
        &env,
        &stake.utoken,
        native_denoms.unwrap_or_default(),
        cw20_assets.unwrap_or_default(),
    )?;

    // 2. Prepare LP withdrawals / deconstruction
    let withdrawals =
        state.get_or_preset(deps.storage, withdrawals, &state.withdrawals_preset, &sender)?;
    let withdrawal_msg = withdrawals.map(|withdrawals| CallbackMsg::WithdrawLps {
        withdrawals,
    });

    // 3. Prepare swap stages
    let stages = state.get_or_preset(deps.storage, stages, &state.stages_preset, &sender)?;
    validate_no_utoken_or_ustake_swap(&env, &stages, &stake)?;

    let swap_msgs = stages.map(|stages| {
        stages
            .into_iter()
            .map(|stage| CallbackMsg::SingleStageSwap {
                stage,
            })
            .collect_vec()
    });

    let multi_swap_router_msg = if let Some(router) = router {
        state.assert_operator(deps.storage, &sender)?;
        validate_no_utoken_or_ustake_coins(&env, &router.1, &stake)?;
        Some(CallbackMsg::MultiSwapRouter {
            router,
        })
    } else {
        None
    };

    Ok(Response::new()
        // 1. Withdraw rewards
        .add_messages(claim_msgs)
        // 2. Withdraw / Destruct LPs
        .add_optional_callback(&env, withdrawal_msg)?
        // 3. swap - multiple single stage swaps
        .add_optional_callbacks(&env, swap_msgs)?
        // 4. swap - single multi swap router
        .add_optional_callback(&env, multi_swap_router_msg)?
        // 5. apply received total utoken to unlocked_coins
        .add_message(check_received_coin_msg(
            &deps,
            &env,
            state.stake_token.load(deps.storage)?,
            None,
        )?)
        // 5. restake unlocked_coins
        .add_callback(&env, CallbackMsg::Reinvest {})?
        .add_attribute("action", "erishub/harvest"))
}

/// this method will split LP positions into each single position
pub fn withdraw_lps(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    withdrawals: Vec<(WithdrawType, DenomType)>,
) -> ContractResult {
    let mut withdraw_msgs: Vec<CosmosMsg<CustomMsgType>> = vec![];
    let chain = chain(&env);
    let get_denoms = || withdrawals.iter().map(|a| a.1.clone()).collect_vec();
    let balances = get_balances_hashmap(&deps, env, get_denoms)?;

    for (withdraw_type, denom) in withdrawals {
        let balance = balances.get(&denom.to_string());

        if let Some(balance) = balance {
            if !balance.is_zero() {
                let msg = chain.create_withdraw_msg(withdraw_type, denom, *balance)?;
                if let Some(msg) = msg {
                    withdraw_msgs.push(msg);
                }
            }
        }
    }

    Ok(Response::new().add_messages(withdraw_msgs).add_attribute("action", "erishub/withdraw_lps"))
}

/// swaps all unlocked coins to token
pub fn single_stage_swap(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    stage: Vec<SingleSwapConfig>,
) -> ContractResult {
    let state = State::default();
    let chain = chain(&env);
    let default_max_spread = state.get_default_max_spread(deps.storage);
    let get_denoms = || stage.iter().map(|a| a.1.clone()).collect_vec();
    let balances = get_balances_hashmap(&deps, env, get_denoms)?;

    let mut response = Response::new().add_attribute("action", "erishub/single_stage_swap");
    // iterate all specified swaps of the stage
    for (stage_type, denom, belief_price, max_amount) in stage {
        let balance = balances.get(&denom.to_string());
        // check if the swap also has a balance in the contract
        if let Some(balance) = balance {
            if !balance.is_zero() {
                let used_amount = match max_amount {
                    Some(max_amount) => {
                        if max_amount.is_zero() {
                            *balance
                        } else {
                            cmp::min(*balance, max_amount)
                        }
                    },
                    None => *balance,
                };

                // create a single swap message add add to submsgs
                let msg = chain.create_single_stage_swap_msgs(
                    stage_type,
                    denom,
                    used_amount,
                    belief_price,
                    default_max_spread,
                )?;
                response = response.add_message(msg)
            }
        }
    }

    Ok(response)
}

/// swaps all unlocked coins to token
pub fn multi_swap_router(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    router: MultiSwapRouter,
) -> ContractResult {
    let state = State::default();
    let stake_token = state.stake_token.load(deps.storage)?;
    let stake_token_denom_native = native_asset_info(stake_token.denom.clone());
    let chain = chain(&env);

    let get_denoms = || router.1.clone();
    let balances = get_balances_hashmap(&deps, env, get_denoms)?;

    let mut response = Response::new().add_attribute("action", "erishub/multi_swap_router");

    let mut coins: Vec<CoinType> = vec![];

    for denom in router.1 {
        // validate that no swap token is already the expected one.
        if chain.equals_asset_info(&denom, &stake_token.utoken)
            || chain.equals_asset_info(&denom, &stake_token_denom_native)
        {
            return Err(ContractError::SwapFromNotAllowed(denom.to_string()));
        }

        let balance = balances.get(&denom.to_string()).copied().unwrap_or_default();
        if !balance.is_zero() {
            let coin = chain.get_coin(denom, balance);
            coins.push(coin);
        }
    }

    if !coins.is_empty() {
        response = response.add_messages(chain.create_multi_swap_router_msgs(router.0, coins)?);
    }

    Ok(response)
}

#[allow(clippy::cmp_owned)]
fn validate_no_utoken_or_ustake_swap(
    env: &Env,
    stages: &Option<Vec<Vec<SingleSwapConfig>>>,
    stake_token: &StakeToken,
) -> Result<(), ContractError> {
    let chain = chain(env);
    let stake_token_denom_native = native_asset_info(stake_token.denom.clone());
    if let Some(stages) = stages {
        for stage in stages {
            for (_addr, denom, _, _) in stage {
                if chain.equals_asset_info(denom, &stake_token.utoken)
                    || chain.equals_asset_info(denom, &stake_token_denom_native)
                {
                    return Err(ContractError::SwapFromNotAllowed(denom.to_string()));
                }
            }
        }
    }
    Ok(())
}

#[allow(clippy::cmp_owned)]
fn validate_no_utoken_or_ustake_coins(
    env: &Env,
    denoms: &Vec<DenomType>,
    stake_token: &StakeToken,
) -> Result<(), ContractError> {
    let chain = chain(env);
    let stake_token_denom_native = native_asset_info(stake_token.denom.clone());
    for denom in denoms {
        if chain.equals_asset_info(denom, &stake_token.utoken)
            || chain.equals_asset_info(denom, &stake_token_denom_native)
        {
            return Err(ContractError::SwapFromNotAllowed(denom.to_string()));
        }
    }
    Ok(())
}

fn validate_no_belief_price(stages: &Vec<Vec<SingleSwapConfig>>) -> Result<(), ContractError> {
    for stage in stages {
        for (_, _, belief_price, _) in stage {
            if belief_price.is_some() {
                return Err(ContractError::BeliefPriceNotAllowed {});
            }
        }
    }
    Ok(())
}

/// This callback is used to take a current snapshot of the balance and add the received balance to the unlocked_coins state after the execution
fn check_received_coin_msg(
    deps: &DepsMut<CustomQueryType>,
    env: &Env,
    stake: StakeToken,
    // offset to account for funds being sent that should be ignored
    negative_offset: Option<Uint128>,
) -> StdResult<CosmosMsg<CustomMsgType>> {
    let mut amount = stake.utoken.query_pool(&deps.querier, env.contract.address.to_string())?;

    if let Some(negative_offset) = negative_offset {
        amount = amount.checked_sub(negative_offset)?;
    }

    let amount_stake =
        deps.querier.query_balance(env.contract.address.to_string(), stake.denom.clone())?.amount;

    CallbackMsg::CheckReceivedCoin {
        // 0. take current balance - offset
        snapshot: stake.utoken.with_balance(amount),
        snapshot_stake: native_asset(stake.denom, amount_stake),
    }
    .into_cosmos_msg(&env.contract.address)
}

fn assert_received_amount_msg(
    deps: &DepsMut<CustomQueryType>,
    env: &Env,
    stake: &StakeToken,
    utoken_expected_received: Uint128,
) -> StdResult<Option<CosmosMsg<CustomMsgType>>> {
    if utoken_expected_received.is_zero() {
        // if nothing is expected to be received, no need to check.
        return Ok(None);
    }

    let expected = stake
        .utoken
        .query_pool(&deps.querier, env.contract.address.to_string())?
        .checked_add(utoken_expected_received)?;

    Ok(Some(
        CallbackMsg::AssertBalance {
            expected: stake.utoken.with_balance(expected),
        }
        .into_cosmos_msg(&env.contract.address)?,
    ))
}

/// NOTE:
/// 1. When delegation Token here, we don't need to use a `SubMsg` to handle the received coins,
///    because we have already withdrawn all claimable staking rewards previously in the same atomic
///    execution.
/// 2. Same as with `bond`, in the latest implementation we only delegate staking rewards with the
///    validator that has the smallest delegation amount.
pub fn reinvest(deps: DepsMut<CustomQueryType>, env: Env) -> ContractResult {
    let state = State::default();
    let fee_config = state.fee_config.load(deps.storage)?;
    let mut unlocked_coins = state.unlocked_coins.load(deps.storage)?;
    let mut stake = state.stake_token.load(deps.storage)?;

    if unlocked_coins.is_empty() {
        return Err(ContractError::NoTokensAvailable(format!(
            "{0}, {1}",
            stake.utoken, stake.denom
        )));
    }

    let mut event = Event::new("erishub/harvested");
    let mut msgs: Vec<CosmosMsg<CustomMsgType>> = vec![];

    let stake_token_denom_native = native_asset_info(stake.denom.clone());

    for asset in unlocked_coins.iter() {
        let available = asset.amount;
        let protocol_fee = fee_config.protocol_reward_fee.checked_mul_uint(available)?;
        let remaining = available.saturating_sub(protocol_fee);

        let send_fee = if asset.info == stake.utoken {
            let to_bond = remaining;

            stake.total_utoken_bonded += to_bond;

            event = event
                .add_attribute("utoken_bonded", to_bond)
                .add_attribute("utoken_protocol_fee", protocol_fee);

            msgs.push(stake.dao_interface.deposit_msg(
                &stake.utoken,
                to_bond,
                env.contract.address.to_string(),
            )?);
            true
        } else if asset.info == stake_token_denom_native {
            // if receiving ustake (staked utoken) -> burn
            event = event
                .add_attribute("ustake_burned", remaining)
                .add_attribute("ustake_protocol_fee", protocol_fee);

            stake.total_supply = stake.total_supply.checked_sub(remaining)?;
            msgs.push(chain(&env).create_burn_msg(stake.denom.clone(), remaining));
            true
        } else {
            // we can ignore other coins as we will only store utoken and ustake there
            false
        };

        if send_fee && !protocol_fee.is_zero() {
            let send_fee = asset
                .info
                .with_balance(protocol_fee)
                .transfer_msg(&fee_config.protocol_fee_contract)?;
            msgs.push(send_fee);
        }
    }

    state.stake_token.save(deps.storage, &stake)?;

    // remove the converted coins. Unlocked_coins track utoken ([TOKEN]) and ustake (amp[TOKEN]).
    unlocked_coins
        .retain(|coin| coin.info != stake.utoken && coin.info != stake_token_denom_native);
    state.unlocked_coins.save(deps.storage, &unlocked_coins)?;

    // update exchange_rate history
    let exchange_rate = calc_current_exchange_rate(stake)?;
    state.exchange_history.save(deps.storage, env.block.time.seconds(), &exchange_rate)?;

    Ok(Response::new()
        .add_messages(msgs)
        .add_event(event)
        .add_attribute("action", "erishub/reinvest")
        .add_attribute("exchange_rate", exchange_rate.to_string()))
}

fn calc_current_exchange_rate(stake: StakeToken) -> Result<Decimal, ContractError> {
    let exchange_rate = if stake.total_supply.is_zero() {
        Decimal::one()
    } else {
        Decimal::from_ratio(stake.total_utoken_bonded, stake.total_supply)
    };
    Ok(exchange_rate)
}

pub fn callback_assert_balance(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    expected: Asset,
) -> ContractResult {
    let current = expected.info.query_pool(&deps.querier, env.contract.address)?;

    if current < expected.amount {
        return Err(ContractError::ExpectingBalance(expected.amount, current));
    }

    Ok(Response::new().add_attribute("action", "erishub/callback_assert_balance"))
}

pub fn callback_received_coins(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    snapshot: Asset,
    snapshot_stake: Asset,
) -> ContractResult {
    let state = State::default();
    // in some cosmwasm versions the events are not received in the callback
    // so each time the contract can receive some coins from rewards we also need to check after receiving some and add them to the unlocked_coins

    let mut received_coins = Assets(vec![]);
    let mut event = Event::new("erishub/received");

    event = event.add_optional_attribute(add_to_received_coins(
        &deps,
        env.contract.address.clone(),
        snapshot,
        &mut received_coins,
    )?);

    event = event.add_optional_attribute(add_to_received_coins(
        &deps,
        env.contract.address,
        snapshot_stake,
        &mut received_coins,
    )?);

    if !received_coins.0.is_empty() {
        state.unlocked_coins.update(deps.storage, |coins| -> StdResult<_> {
            let mut coins = Assets(coins);
            coins.add_many(&received_coins)?;
            Ok(coins.0)
        })?;
    }

    Ok(Response::new().add_event(event).add_attribute("action", "erishub/received"))
}

fn add_to_received_coins(
    deps: &DepsMut<CustomQueryType>,
    contract: Addr,
    snapshot: Asset,
    received_coins: &mut Assets,
) -> Result<Option<Attribute>, ContractError> {
    let current_balance = snapshot.info.query_pool(&deps.querier, contract)?;

    let attr = if current_balance > snapshot.amount {
        let received_amount = current_balance.checked_sub(snapshot.amount)?;
        let received = snapshot.info.with_balance(received_amount);
        received_coins.add(&received)?;
        Some(attr("received_coin", received.to_string()))
    } else {
        None
    };

    Ok(attr)
}

//--------------------------------------------------------------------------------------------------
// Unbonding logics
//--------------------------------------------------------------------------------------------------

pub fn queue_unbond(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    stake: StakeToken,
    receiver: Addr,
    ustake_to_burn: Uint128,
) -> ContractResult {
    assert_not_disabled(&stake)?;

    let state = State::default();
    let mut pending_batch = state.pending_batch.load(deps.storage)?;
    pending_batch.ustake_to_burn += ustake_to_burn;
    state.pending_batch.save(deps.storage, &pending_batch)?;

    state.unbond_requests.update(
        deps.storage,
        (pending_batch.id, &receiver),
        |x| -> StdResult<_> {
            let mut request = x.unwrap_or_else(|| UnbondRequest {
                id: pending_batch.id,
                user: receiver.clone(),
                shares: Uint128::zero(),
            });
            request.shares += ustake_to_burn;
            Ok(request)
        },
    )?;

    let mut msgs: Vec<CosmosMsg<CustomMsgType>> = vec![];
    let mut start_time = pending_batch.est_unbond_start_time.to_string();
    if env.block.time.seconds() > pending_batch.est_unbond_start_time {
        start_time = "immediate".to_string();
        msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.into(),
            msg: to_json_binary(&ExecuteMsg::SubmitBatch {})?,
            funds: vec![],
        }));
    }

    let event = Event::new("erishub/unbond_queued")
        .add_attribute("est_unbond_start_time", start_time)
        .add_attribute("id", pending_batch.id.to_string())
        .add_attribute("receiver", receiver)
        .add_attribute("ustake_to_burn", ustake_to_burn);

    Ok(Response::new()
        .add_messages(msgs)
        .add_event(event)
        .add_attribute("action", "erishub/queue_unbond"))
}

// is allowed as denom can require a clone based on the chain
#[allow(clippy::redundant_clone)]
pub fn submit_batch(deps: DepsMut<CustomQueryType>, env: Env) -> ContractResult {
    let state = State::default();
    let mut stake = state.stake_token.load(deps.storage)?;
    assert_not_disabled(&stake)?;
    let unbond_period = state.unbond_period.load(deps.storage)?;
    let pending_batch = state.pending_batch.load(deps.storage)?;

    let current_time = env.block.time.seconds();
    if current_time < pending_batch.est_unbond_start_time {
        return Err(ContractError::SubmitBatchAfter(pending_batch.est_unbond_start_time));
    }

    let ustake_supply = stake.total_supply;

    let utoken_to_unbond = compute_unbond_amount(
        ustake_supply,
        pending_batch.ustake_to_burn,
        stake.total_utoken_bonded,
    );

    state.previous_batches.save(
        deps.storage,
        pending_batch.id,
        &Batch {
            id: pending_batch.id,
            reconciled: false,
            total_shares: pending_batch.ustake_to_burn,
            utoken_unclaimed: utoken_to_unbond,
            est_unbond_end_time: current_time + unbond_period,
        },
    )?;

    let epoch_period = state.epoch_period.load(deps.storage)?;
    state.pending_batch.save(
        deps.storage,
        &PendingBatch {
            id: pending_batch.id + 1,
            ustake_to_burn: Uint128::zero(),
            est_unbond_start_time: current_time + epoch_period,
        },
    )?;

    let unbond_msg = stake.dao_interface.unbond_msg(&stake.utoken, utoken_to_unbond)?;

    // apply burn to the stored total supply and save state
    stake.total_utoken_bonded = stake.total_utoken_bonded.checked_sub(utoken_to_unbond)?;
    stake.total_supply = stake.total_supply.checked_sub(pending_batch.ustake_to_burn)?;
    state.stake_token.save(deps.storage, &stake)?;

    let burn_msg: CosmosMsg<CustomMsgType> =
        chain(&env).create_burn_msg(stake.denom.clone(), pending_batch.ustake_to_burn);

    let event = Event::new("erishub/unbond_submitted")
        .add_attribute("id", pending_batch.id.to_string())
        .add_attribute("utoken_unbonded", utoken_to_unbond)
        .add_attribute("ustake_burned", pending_batch.ustake_to_burn);

    Ok(Response::new()
        .add_message(unbond_msg)
        .add_message(burn_msg)
        // .add_message(check_received_coin_msg(&deps, &env, stake, None)?)
        .add_event(event)
        .add_attribute("action", "erishub/unbond"))
}

pub fn reconcile(deps: DepsMut<CustomQueryType>, env: Env) -> ContractResult {
    let state = State::default();
    let stake = state.stake_token.load(deps.storage)?;
    assert_not_disabled(&stake)?;
    let current_time = env.block.time.seconds();

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

    let batches = all_batches
        .into_iter()
        .filter(|b| current_time > b.est_unbond_end_time)
        .collect::<Vec<_>>();

    let utoken_expected_received: Uint128 = batches.iter().map(|b| b.utoken_unclaimed).sum();

    if utoken_expected_received.is_zero() {
        return Ok(Response::new());
    }

    let mut ids: Vec<String> = vec![];
    for mut batch in batches {
        batch.reconciled = true;
        ids.push(batch.id.to_string());
        state.previous_batches.save(deps.storage, batch.id, &batch)?;
    }

    let ids = ids.join(",");
    let event = Event::new("erishub/reconciled")
        .add_attribute("ids", ids)
        .add_attribute("utoken_deducted", "0");

    Ok(Response::new()
        .add_message(stake.dao_interface.claim_unbonded_msg()?)
        // validate that the amount received is the one expected - otherwise dont allow reconciliation
        .add_optional_message(assert_received_amount_msg(
            &deps,
            &env,
            &stake,
            utoken_expected_received,
        )?)
        .add_event(event)
        .add_attribute("action", "erishub/reconcile"))
}

pub fn withdraw_unbonded(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    user: Addr,
    receiver: Addr,
) -> ContractResult {
    let state = State::default();
    let current_time = env.block.time.seconds();

    let stake = state.stake_token.load(deps.storage)?;
    assert_not_disabled(&stake)?;

    // NOTE: If the user has too many unclaimed requests, this may not fit in the WASM memory...
    // However, this is practically never going to happen. Who would create hundreds of unbonding
    // requests and never claim them?
    let requests = state
        .unbond_requests
        .idx
        .user
        .prefix(user.to_string())
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| {
            let (_, v) = item?;
            Ok(v)
        })
        .collect::<StdResult<Vec<_>>>()?;

    // NOTE: Token in the following batches are withdrawn it the batch:
    // - is a _previous_ batch, not a _pending_ batch
    // - is reconciled
    // - has finished unbonding
    // If not sure whether the batches have been reconciled, the user should first invoke `ExecuteMsg::Reconcile`
    // before withdrawing.
    let mut total_utoken_to_refund = Uint128::zero();
    let mut ids: Vec<String> = vec![];
    for request in &requests {
        if let Ok(mut batch) = state.previous_batches.load(deps.storage, request.id) {
            if batch.reconciled && batch.est_unbond_end_time < current_time {
                let utoken_to_refund =
                    batch.utoken_unclaimed.multiply_ratio(request.shares, batch.total_shares);

                ids.push(request.id.to_string());

                total_utoken_to_refund += utoken_to_refund;
                batch.total_shares -= request.shares;
                batch.utoken_unclaimed -= utoken_to_refund;

                if batch.total_shares.is_zero() {
                    state.previous_batches.remove(deps.storage, request.id)?;
                } else {
                    state.previous_batches.save(deps.storage, batch.id, &batch)?;
                }

                state.unbond_requests.remove(deps.storage, (request.id, &user))?;
            }
        }
    }

    if total_utoken_to_refund.is_zero() {
        return Err(ContractError::CantBeZero("withdrawable amount".into()));
    }

    let refund_msg = stake.utoken.with_balance(total_utoken_to_refund).transfer_msg(&receiver)?;

    let event = Event::new("erishub/unbonded_withdrawn")
        .add_attribute("ids", ids.join(","))
        .add_attribute("user", user)
        .add_attribute("receiver", receiver)
        .add_attribute("utoken_refunded", total_utoken_to_refund);

    Ok(Response::new()
        .add_message(refund_msg)
        .add_event(event)
        .add_attribute("action", "erishub/withdraw_unbonded"))
}

//--------------------------------------------------------------------------------------------------
// Ownership and management logics
//--------------------------------------------------------------------------------------------------

pub fn transfer_ownership(
    deps: DepsMut<CustomQueryType>,
    sender: Addr,
    new_owner: String,
) -> ContractResult {
    let state = State::default();

    state.assert_owner(deps.storage, &sender)?;
    state.new_owner.save(deps.storage, &deps.api.addr_validate(&new_owner)?)?;

    Ok(Response::new().add_attribute("action", "erishub/transfer_ownership"))
}

pub fn drop_ownership_proposal(deps: DepsMut<CustomQueryType>, sender: Addr) -> ContractResult {
    let state = State::default();

    state.assert_owner(deps.storage, &sender)?;
    state.new_owner.remove(deps.storage);

    Ok(Response::new().add_attribute("action", "erishub/drop_ownership_proposal"))
}

pub fn accept_ownership(deps: DepsMut<CustomQueryType>, sender: Addr) -> ContractResult {
    let state = State::default();

    let previous_owner = state.owner.load(deps.storage)?;
    let new_owner = state.new_owner.load(deps.storage)?;

    if sender != new_owner {
        return Err(ContractError::UnauthorizedSenderNotNewOwner {});
    }

    state.owner.save(deps.storage, &sender)?;
    state.new_owner.remove(deps.storage);

    let event = Event::new("erishub/ownership_transferred")
        .add_attribute("new_owner", new_owner)
        .add_attribute("previous_owner", previous_owner);

    Ok(Response::new().add_event(event).add_attribute("action", "erishub/transfer_ownership"))
}

#[allow(clippy::too_many_arguments)]
pub fn update_config(
    env: Env,
    deps: DepsMut<CustomQueryType>,
    sender: Addr,
    protocol_fee_contract: Option<String>,
    protocol_reward_fee: Option<Decimal>,
    operator: Option<String>,
    stages_preset: Option<Vec<Vec<SingleSwapConfig>>>,
    withdrawals_preset: Option<Vec<(WithdrawType, DenomType)>>,
    allow_donations: Option<bool>,
    vote_operator: Option<String>,
    default_max_spread: Option<u64>,
    epoch_period: Option<u64>,
    unbond_period: Option<u64>,
    dao_interface: Option<DaoInterface<String>>,
) -> ContractResult {
    let state = State::default();

    state.assert_owner(deps.storage, &sender)?;

    if protocol_fee_contract.is_some() || protocol_reward_fee.is_some() {
        let mut fee_config = state.fee_config.load(deps.storage)?;

        if let Some(protocol_fee_contract) = protocol_fee_contract {
            fee_config.protocol_fee_contract = deps.api.addr_validate(&protocol_fee_contract)?;
        }

        if let Some(protocol_reward_fee) = protocol_reward_fee {
            if protocol_reward_fee.gt(&get_reward_fee_cap()) {
                return Err(ContractError::ProtocolRewardFeeTooHigh {});
            }
            fee_config.protocol_reward_fee = protocol_reward_fee;
        }

        state.fee_config.save(deps.storage, &fee_config)?;
    }

    if let Some(epoch_period) = epoch_period {
        if epoch_period == 0 {
            return Err(ContractError::CantBeZero("epoch_period".into()));
        }
        state.epoch_period.save(deps.storage, &epoch_period)?;
    }

    if let Some(unbond_period) = unbond_period {
        if unbond_period == 0 {
            return Err(ContractError::CantBeZero("unbond_period".into()));
        }
        state.unbond_period.save(deps.storage, &unbond_period)?;
    }

    if let Some(operator) = operator {
        state.operator.save(deps.storage, &deps.api.addr_validate(operator.as_str())?)?;
    }

    if let Some(dao_interface) = dao_interface {
        let mut stake = state.stake_token.load(deps.storage)?;
        stake.dao_interface = dao_interface.validate(deps.api)?;
        state.stake_token.save(deps.storage, &stake)?;
    }

    if stages_preset.is_some() {
        validate_no_utoken_or_ustake_swap(
            &env,
            &stages_preset,
            &state.stake_token.load(deps.storage)?,
        )?;
    }

    if let Some(stages_preset) = stages_preset {
        // belief price is not allowed. We still store it with None, as otherwise a lot of additional logic is required to load it.
        validate_no_belief_price(&stages_preset)?;
        state.stages_preset.save(deps.storage, &stages_preset)?;
    }

    if let Some(withdrawals_preset) = withdrawals_preset {
        state.withdrawals_preset.save(deps.storage, &withdrawals_preset)?;
    }

    if let Some(allow_donations) = allow_donations {
        state.allow_donations.save(deps.storage, &allow_donations)?;
    }
    if let Some(default_max_spread) = default_max_spread {
        state.default_max_spread.save(deps.storage, &default_max_spread)?;
    }

    if let Some(vote_operator) = vote_operator {
        state.vote_operator.save(deps.storage, &deps.api.addr_validate(&vote_operator)?)?;
    }

    Ok(Response::new().add_attribute("action", "erishub/update_config"))
}
