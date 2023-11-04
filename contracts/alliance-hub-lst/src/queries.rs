use std::ops::Div;

use astroport::asset::native_asset_info;
use cosmwasm_std::{Decimal, Deps, Env, Order, StdResult};
use cw_storage_plus::Bound;

use eris::hub_alliance::{ConfigResponse, ExchangeRatesResponse, PairInfo, StateResponse};
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
        fee_config: state.fee_config.load(deps.storage)?,
        stages_preset: state.stages_preset.may_load(deps.storage)?.unwrap_or_default(),
        withdrawals_preset: state.withdrawals_preset.may_load(deps.storage)?.unwrap_or_default(),
        allow_donations: state.allow_donations.may_load(deps.storage)?.unwrap_or(false),
        dao_interface: stake.dao_interface,
    })
}

pub fn state(deps: Deps<CustomQueryType>, env: Env) -> StdResult<StateResponse> {
    let state = State::default();

    let stake_token = state.stake_token.load(deps.storage)?;
    let total_ustake = stake_token.total_supply;
    let total_utoken = stake_token.total_utoken_bonded;

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
        available,
        tvl_utoken: total_utoken.checked_add(available)?,
    })
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

pub fn query_pair(deps: Deps<CustomQueryType>, env: Env) -> StdResult<PairInfo> {
    let state = State::default();
    let stake_token = state.stake_token.load(deps.storage)?;

    Ok(PairInfo {
        asset_infos: vec![stake_token.utoken, native_asset_info(stake_token.denom)],
        contract_addr: env.contract.address.clone(),
        liquidity_token: env.contract.address,
        pair_type: eris::hub_alliance::PairType::Custom("virtual".to_string()),
    })
}
