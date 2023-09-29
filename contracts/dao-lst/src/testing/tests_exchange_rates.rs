use super::helpers::mock_env_at_timestamp;
use crate::constants::DAY;
use crate::contract::execute;
use crate::state::State;
use crate::testing::helpers::{get_stake_full_denom, query_helper_env, setup_test, MOCK_UTOKEN};
use astroport::asset::{native_asset_info, AssetInfoExt};
use cosmwasm_std::testing::{mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{attr, Uint128};
use eris::hub::{CallbackMsg, ExchangeRatesResponse, ExecuteMsg, QueryMsg, StakeToken};

//--------------------------------------------------------------------------------------------------
// Execution
//--------------------------------------------------------------------------------------------------

#[test]
fn reinvesting_check_exchange_rates() {
    let state = State::default();

    let (mut deps, mut stake) = setup_test();
    stake.total_supply = Uint128::new(100_000);
    stake.total_utoken_bonded = Uint128::new(1_000_000);
    state.stake_token.save(deps.as_mut().storage, &stake).unwrap();

    // After the swaps, `unlocked_coins` should contain only utoken and unknown denoms
    state
        .unlocked_coins
        .save(
            deps.as_mut().storage,
            &vec![
                native_asset_info(MOCK_UTOKEN.to_string()).with_balance(234u128),
                native_asset_info(get_stake_full_denom()).with_balance(111u128),
            ],
        )
        .unwrap();

    let res = execute(
        deps.as_mut(),
        mock_env_at_timestamp(0),
        mock_info(MOCK_CONTRACT_ADDR, &[]),
        ExecuteMsg::Callback(CallbackMsg::Reinvest {}),
    )
    .unwrap();
    assert_eq!(res.messages.len(), 4);

    // ustake: (0_100000 - (111 - fees)), utoken: 1_000000 + (234 - fees)
    assert_eq!(
        res.attributes,
        vec![attr("action", "erishub/reinvest"), attr("exchange_rate", "10.013334668134948443")]
    );

    let stake = state.stake_token.load(deps.as_mut().storage).unwrap();
    assert_eq!(
        stake,
        StakeToken {
            total_supply: Uint128::new(100000 - 110), // only 110 because of fee
            total_utoken_bonded: Uint128::new(1000232),

            utoken: stake.utoken.clone(),
            denom: stake.denom.clone(),
            dao_interface: stake.dao_interface.clone(),
        }
    );

    // added delegation of 234 - fees

    state
        .unlocked_coins
        .save(
            deps.as_mut().storage,
            &vec![
                native_asset_info(MOCK_UTOKEN.to_string()).with_balance(200u128),
                native_asset_info(get_stake_full_denom()).with_balance(300u128),
            ],
        )
        .unwrap();

    let res = execute(
        deps.as_mut(),
        mock_env_at_timestamp(DAY),
        mock_info(MOCK_CONTRACT_ADDR, &[]),
        ExecuteMsg::Callback(CallbackMsg::Reinvest {}),
    )
    .unwrap();
    assert_eq!(res.messages.len(), 4);

    let res: ExchangeRatesResponse = query_helper_env(
        deps.as_ref(),
        QueryMsg::ExchangeRates {
            start_after: None,
            limit: None,
        },
        2083600,
    );
    assert_eq!(
        res.exchange_rates
            .into_iter()
            .map(|a| format!("{0};{1}", a.0, a.1))
            .collect::<Vec<String>>(),
        vec!["86400;10.045183898466759712".to_string(), "0;10.013334668134948443".to_string()]
    );

    // 10.013334668134948443 -> 10.045183898466759712 within 1 day
    assert_eq!(res.apr.map(|a| a.to_string()), Some("0.003180681699690299".to_string()));
}
