use std::ops::Div;

use cosmwasm_std::{Addr, Decimal, Deps, Env, Order, StdResult, Uint128};
use cw_storage_plus::Bound;

use eris::hub::{
    Batch, ConfigResponse, ExchangeRatesResponse, PendingBatch, StateResponse,
    UnbondRequestsByBatchResponseItem, UnbondRequestsByUserResponseItem,
    UnbondRequestsByUserResponseItemDetails,
};
use eris_chain_adapter::types::CustomQueryType;

use crate::constants::DAY;
use crate::state::State;

const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

pub fn config(deps: Deps<CustomQueryType>) -> StdResult<ConfigResponse> {
    let state = State::default();

    let stake = state.stake_token.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: state.owner.load(deps.storage)?.into(),
        operator: state.operator.load(deps.storage)?.into(),
        new_owner: state.new_owner.may_load(deps.storage)?.map(|addr| addr.into()),
        utoken: stake.utoken,
        stake_token: stake.denom,
        epoch_period: state.epoch_period.load(deps.storage)?,
        unbond_period: state.unbond_period.load(deps.storage)?,
        fee_config: state.fee_config.load(deps.storage)?,
        stages_preset: state.stages_preset.may_load(deps.storage)?.unwrap_or_default(),
        withdrawals_preset: state.withdrawals_preset.may_load(deps.storage)?.unwrap_or_default(),
        allow_donations: state.allow_donations.may_load(deps.storage)?.unwrap_or(false),
        vote_operator: state.vote_operator.may_load(deps.storage)?.map(|addr| addr.into()),
        dao_interface: stake.dao_interface,
    })
}

pub fn state(deps: Deps<CustomQueryType>, env: Env) -> StdResult<StateResponse> {
    let state = State::default();

    let stake_token = state.stake_token.load(deps.storage)?;
    let total_ustake = stake_token.total_supply;
    let total_utoken = stake_token.total_utoken_bonded;

    // only not reconciled batches are relevant as they are still unbonding and estimated unbond time in the future.
    let unbonding: u128 = state
        .previous_batches
        .idx
        .reconciled
        .prefix(false.into())
        .range(deps.storage, None, None, Order::Descending)
        .map(|item| {
            let (_, v) = item.unwrap();
            v
        })
        .map(|item| item.utoken_unclaimed.u128())
        .sum();

    let available = stake_token.utoken.query_pool(&deps.querier, env.contract.address)?;

    let exchange_rate = if total_ustake.is_zero() {
        Decimal::one()
    } else {
        Decimal::from_ratio(total_utoken, total_ustake)
    };

    Ok(StateResponse {
        total_ustake,
        total_utoken,
        exchange_rate,
        unlocked_coins: state.unlocked_coins.load(deps.storage)?,
        unbonding: Uint128::from(unbonding),
        available,
        tvl_utoken: total_utoken.checked_add(Uint128::from(unbonding))?.checked_add(available)?,
    })
}

pub fn pending_batch(deps: Deps<CustomQueryType>) -> StdResult<PendingBatch> {
    let state = State::default();
    state.pending_batch.load(deps.storage)
}

pub fn previous_batch(deps: Deps<CustomQueryType>, id: u64) -> StdResult<Batch> {
    let state = State::default();
    state.previous_batches.load(deps.storage, id)
}

pub fn previous_batches(
    deps: Deps<CustomQueryType>,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<Vec<Batch>> {
    let state = State::default();

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    state
        .previous_batches
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (_, v) = item?;
            Ok(v)
        })
        .collect()
}

pub fn unbond_requests_by_batch(
    deps: Deps<CustomQueryType>,
    id: u64,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Vec<UnbondRequestsByBatchResponseItem>> {
    let state = State::default();

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    let mut start: Option<Bound<&Addr>> = None;
    let addr: Addr;
    if let Some(start_after) = start_after {
        if let Ok(start_after_addr) = deps.api.addr_validate(&start_after) {
            addr = start_after_addr;
            start = Some(Bound::exclusive(&addr));
        }
    }

    state
        .unbond_requests
        .prefix(id)
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (_, v) = item?;
            Ok(v.into())
        })
        .collect()
}

pub fn unbond_requests_by_user(
    deps: Deps<CustomQueryType>,
    user: String,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<Vec<UnbondRequestsByUserResponseItem>> {
    let state = State::default();

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let addr = deps.api.addr_validate(&user)?;
    let start = start_after.map(|id| Bound::exclusive((id, &addr)));

    state
        .unbond_requests
        .idx
        .user
        .prefix(user)
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (_, v) = item?;

            Ok(v.into())
        })
        .collect()
}

pub fn unbond_requests_by_user_details(
    deps: Deps<CustomQueryType>,
    user: String,
    start_after: Option<u64>,
    limit: Option<u32>,
    env: Env,
) -> StdResult<Vec<UnbondRequestsByUserResponseItemDetails>> {
    let state = State::default();

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let addr = deps.api.addr_validate(&user)?;
    let start = start_after.map(|id| Bound::exclusive((id, &addr)));

    let pending = state.pending_batch.load(deps.storage)?;

    state
        .unbond_requests
        .idx
        .user
        .prefix(user)
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (_, v) = item?;

            let state_msg: String;
            let previous: Option<Batch>;
            if pending.id == v.id {
                state_msg = "PENDING".to_string();
                previous = None;
            } else {
                let batch = state.previous_batches.load(deps.storage, v.id)?;
                previous = Some(batch.clone());
                let current_time = env.block.time.seconds();
                state_msg = if batch.est_unbond_end_time < current_time {
                    "COMPLETED".to_string()
                } else {
                    "UNBONDING".to_string()
                }
            }

            Ok(UnbondRequestsByUserResponseItemDetails {
                id: v.id,
                shares: v.shares,
                state: state_msg,
                pending: if pending.id == v.id {
                    Some(pending.clone())
                } else {
                    None
                },
                batch: previous,
            })
        })
        .collect()
}

pub fn query_exchange_rates(
    deps: Deps<CustomQueryType>,
    _env: Env,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<ExchangeRatesResponse> {
    let state = State::default();
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let end = start_after.map(Bound::exclusive);

    let exchange_rates = state
        .exchange_history
        .range(deps.storage, None, end, Order::Descending)
        .take(limit)
        .collect::<StdResult<Vec<(u64, Decimal)>>>()?;

    let apr: Option<Decimal> = if exchange_rates.len() > 1 {
        let current = exchange_rates[0];
        let last = exchange_rates[exchange_rates.len() - 1];

        let delta_time_s = current.0 - last.0;
        let delta_rate = current.1.checked_sub(last.1).unwrap_or_default();

        Some(delta_rate.checked_mul(Decimal::from_ratio(DAY, delta_time_s).div(last.1))?)
    } else {
        None
    };

    Ok(ExchangeRatesResponse {
        exchange_rates,
        apr,
    })
}
