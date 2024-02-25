use std::cmp;

use astroport::asset::{native_asset, native_asset_info, Asset, AssetInfoExt};
use cosmwasm_std::{
    attr, Addr, Attribute, CosmosMsg, Decimal, DepsMut, Env, Event, Response, StdResult, Uint128,
};
use cw2::set_contract_version;
use eris::adapters::asset::AssetEx;
use eris::{CustomEvent, CustomResponse, DecimalCheckedOps};

use eris::hub_alliance::{
    CallbackMsg, FeeConfig, InstantiateMsg, MultiSwapRouter, SingleSwapConfig, StakeToken,
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

const CONTRACT_NAME: &str = "eris-alliance-hub-lst";
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

    state.owner.save(deps.storage, &deps.api.addr_validate(&msg.owner)?)?;
    state.operator.save(deps.storage, &deps.api.addr_validate(&msg.operator)?)?;

    // by default donations are set to false
    state.allow_donations.save(deps.storage, &false)?;

    state.fee_config.save(
        deps.storage,
        &FeeConfig {
            protocol_fee_contract: deps.api.addr_validate(&msg.protocol_fee_contract)?,
            protocol_reward_fee: msg.protocol_reward_fee,
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
        },
    )?;

    Ok(Response::new().add_message(chain.create_denom_msg(full_denom, sub_denom)))
}

//--------------------------------------------------------------------------------------------------
// Bonding and harvesting logics
//--------------------------------------------------------------------------------------------------

pub fn bond(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    state: State,
    mut stake: StakeToken,
    token_to_bond: Uint128,

    receiver: Addr,
    donate: bool,
) -> ContractResult {
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

    // 1. Withdraw rewards
    let claim_msg = stake.dao_interface.claim_rewards_msg(
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
        .add_message(claim_msg)
        // 2. Withdraw / Destruct LPs
        .add_optional_callback_alliance(&env, withdrawal_msg)?
        // 3. swap - multiple single stage swaps
        .add_optional_callbacks_alliance(&env, swap_msgs)?
        // 4. swap - single multi swap router
        .add_optional_callback_alliance(&env, multi_swap_router_msg)?
        // 5. apply received total utoken to unlocked_coins
        .add_message(check_received_coin_msg(
            &deps,
            &env,
            state.stake_token.load(deps.storage)?,
            None,
        )?)
        // 5. restake unlocked_coins
        .add_callback_alliance(&env, CallbackMsg::Reinvest {})?
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

/// NOTE:
/// 1. When delegation Token here, we don't need to use a `SubMsg` to handle the received coins,
/// because we have already withdrawn all claimable staking rewards previously in the same atomic
/// execution.
/// 2. Same as with `bond`, in the latest implementation we only delegate staking rewards with the
/// validator that has the smallest delegation amount.
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

pub fn unbond(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    user: Addr,
    receiver: Addr,
    ustake_to_burn: Uint128,
) -> ContractResult {
    let state = State::default();
    let mut stake = state.stake_token.load(deps.storage)?;

    let ustake_supply = stake.total_supply;

    let utoken_to_unbond =
        compute_unbond_amount(ustake_supply, ustake_to_burn, stake.total_utoken_bonded);

    let unbond_msg = stake.dao_interface.unbond_msg(&stake.utoken, utoken_to_unbond)?;
    let burn_msg: CosmosMsg<CustomMsgType> =
        chain(&env).create_burn_msg(stake.denom.clone(), ustake_to_burn);
    let refund_msg = stake.utoken.with_balance(utoken_to_unbond).transfer_msg(&receiver)?;

    // apply burn to the stored total supply and save state
    stake.total_utoken_bonded = stake.total_utoken_bonded.checked_sub(utoken_to_unbond)?;
    stake.total_supply = stake.total_supply.checked_sub(ustake_to_burn)?;
    state.stake_token.save(deps.storage, &stake)?;

    let event = Event::new("erishub/unbonded_withdrawn")
        .add_attribute("user", user)
        .add_attribute("receiver", receiver)
        .add_attribute("ustake_burned", ustake_to_burn)
        .add_attribute("utoken_refunded", utoken_to_unbond);

    Ok(Response::new()
        .add_message(unbond_msg)
        .add_message(burn_msg)
        .add_message(refund_msg)
        .add_event(event)
        .add_attribute("action", "erishub/unbond"))
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
    default_max_spread: Option<u64>,
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

    if let Some(operator) = operator {
        state.operator.save(deps.storage, &deps.api.addr_validate(operator.as_str())?)?;
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

    Ok(Response::new().add_attribute("action", "erishub/update_config"))
}
