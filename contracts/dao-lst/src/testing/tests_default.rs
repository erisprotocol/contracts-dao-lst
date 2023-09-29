use std::ops::Sub;

use astroport::asset::{native_asset_info, AssetInfoExt};
use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    coin, to_binary, Addr, BankMsg, Coin, CosmosMsg, Decimal, Event, Order, StdError, StdResult,
    SubMsg, Uint128, VoteOption, WasmMsg,
};
use eris::DecimalCheckedOps;

use eris::helper::validate_received_funds;
use eris::hub::{
    Batch, CallbackMsg, ConfigResponse, ExecuteMsg, FeeConfig, PendingBatch, QueryMsg, StakeToken,
    StateResponse, UnbondRequest, UnbondRequestsByBatchResponseItem,
    UnbondRequestsByUserResponseItem, UnbondRequestsByUserResponseItemDetails,
};

use eris_chain_shared::chain_trait::ChainInterface;

use crate::contract::execute;
use crate::error::ContractError;
use crate::state::State;
use crate::testing::helpers::{
    chain_test, check_received_coin, get_stake_full_denom, mock_utoken, query_helper_env,
    set_total_stake_supply, setup_test, MOCK_UTOKEN,
};
use crate::testing::WithoutGeneric;

use super::helpers::{mock_dependencies, mock_env_at_timestamp, query_helper};

//--------------------------------------------------------------------------------------------------
// Execution
//--------------------------------------------------------------------------------------------------

#[test]
fn proper_instantiation() {
    let (deps, _) = setup_test();

    let res: ConfigResponse = query_helper(deps.as_ref(), QueryMsg::Config {});
    assert_eq!(
        res,
        ConfigResponse {
            owner: "owner".to_string(),
            new_owner: None,
            stake_token: get_stake_full_denom(),
            epoch_period: 259200,
            unbond_period: 1814400,
            fee_config: FeeConfig {
                protocol_fee_contract: Addr::unchecked("fee"),
                protocol_reward_fee: Decimal::from_ratio(1u128, 100u128)
            },

            operator: "operator".to_string(),
            stages_preset: vec![],
            withdrawals_preset: vec![],
            allow_donations: false,
            vote_operator: None,
            utoken: native_asset_info(MOCK_UTOKEN.to_string())
        }
    );

    let res: StateResponse = query_helper(deps.as_ref(), QueryMsg::State {});
    assert_eq!(
        res,
        StateResponse {
            total_ustake: Uint128::zero(),
            total_utoken: Uint128::zero(),
            exchange_rate: Decimal::one(),
            unlocked_coins: vec![],
            unbonding: Uint128::zero(),
            available: Uint128::zero(),
            tvl_utoken: Uint128::zero(),
        },
    );

    let res: PendingBatch = query_helper(deps.as_ref(), QueryMsg::PendingBatch {});
    assert_eq!(
        res,
        PendingBatch {
            id: 1,
            ustake_to_burn: Uint128::zero(),
            est_unbond_start_time: 269200, // 10,000 + 259,200
        },
    );
}

#[test]
fn bonding() {
    let (mut deps, stake) = setup_test();

    deps.querier.set_bank_balances(&[coin(1000000, MOCK_UTOKEN)]);

    // Bond when no delegation has been made
    // In this case, the full deposit simply goes to the first validator
    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_1", &[Coin::new(1000000, MOCK_UTOKEN)]),
        ExecuteMsg::Bond {
            receiver: None,
            donate: None,
        },
    )
    .unwrap();

    let mint_msgs = chain_test().create_mint_msgs(
        get_stake_full_denom(),
        Uint128::new(1000000),
        Addr::unchecked("user_1"),
    );
    assert_eq!(res.messages.len(), 1 + mint_msgs.len());

    let mut index = 0;
    assert_eq!(res.messages[index].msg, stake.deposit_msg(Uint128::new(1000000)).unwrap());

    index += 1;
    for msg in mint_msgs {
        assert_eq!(res.messages[index].msg, msg);
        index += 1;
    }

    assert_eq!(
        State::default().stake_token.load(deps.as_ref().storage).unwrap(),
        StakeToken {
            utoken: stake.utoken.clone(),
            dao_interface: stake.dao_interface.clone(),
            denom: stake.denom.clone(),
            total_supply: Uint128::new(1000000),
            total_utoken_bonded: Uint128::new(1000000),
        }
    );

    deps.querier.set_bank_balances(&[coin(12345, MOCK_UTOKEN)]);
    // Charlie has the smallest amount of delegation, so the full deposit goes to him
    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_2", &[Coin::new(12345, MOCK_UTOKEN)]),
        ExecuteMsg::Bond {
            receiver: Some("user_3".to_string()),
            donate: Some(false),
        },
    )
    .unwrap();

    let mint_msgs = chain_test().create_mint_msgs(
        get_stake_full_denom(),
        Uint128::new(12345),
        Addr::unchecked("user_3"),
    );
    assert_eq!(res.messages.len(), 1 + mint_msgs.len());

    let mut index = 0;
    assert_eq!(res.messages[index].msg, stake.deposit_msg(Uint128::new(12345)).unwrap());

    index += 1;
    for msg in mint_msgs {
        assert_eq!(res.messages[index].msg, msg);
        index += 1;
    }

    deps.querier.set_bank_balances(&[coin(0, MOCK_UTOKEN)]);
    let res: StateResponse = query_helper(deps.as_ref(), QueryMsg::State {});
    assert_eq!(
        res,
        StateResponse {
            total_ustake: Uint128::new(1012345),
            total_utoken: Uint128::new(1012345),
            exchange_rate: Decimal::from_ratio(1u128, 1u128),
            unlocked_coins: vec![],
            unbonding: Uint128::zero(),
            available: Uint128::new(0),
            tvl_utoken: Uint128::new(1012345),
        }
    );
}

#[test]
fn donating() {
    let (mut deps, stake) = setup_test();

    deps.querier.set_bank_balances(&[coin(1000000, MOCK_UTOKEN)]);
    // Bond when no delegation has been made
    // In this case, the full deposit simply goes to the first validator
    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_1", &[Coin::new(1000000, MOCK_UTOKEN)]),
        ExecuteMsg::Bond {
            receiver: None,
            donate: None,
        },
    )
    .unwrap();

    let mint_msgs = chain_test().create_mint_msgs(
        get_stake_full_denom(),
        Uint128::new(1000000),
        Addr::unchecked("user_1"),
    );
    assert_eq!(res.messages.len(), 1 + mint_msgs.len());

    let mut index = 0;
    assert_eq!(res.messages[index].msg, stake.deposit_msg(Uint128::new(1000000)).unwrap());

    index += 1;
    for msg in mint_msgs {
        assert_eq!(res.messages[index].msg, msg);
        index += 1;
    }

    // deps.querier.set_cw20_total_supply("stake_token", 1000000);

    deps.querier.set_bank_balances(&[coin(0, MOCK_UTOKEN)]);
    let res: StateResponse = query_helper(deps.as_ref(), QueryMsg::State {});
    assert_eq!(
        res,
        StateResponse {
            total_ustake: Uint128::new(1000000),
            total_utoken: Uint128::new(1000000),
            exchange_rate: Decimal::from_ratio(1000000u128, 1000000u128),
            unlocked_coins: vec![],
            unbonding: Uint128::zero(),
            available: Uint128::new(0),
            tvl_utoken: Uint128::new(1000000),
        }
    );

    deps.querier.set_bank_balances(&[coin(12345, MOCK_UTOKEN)]);
    // Charlie has the smallest amount of delegation, so the full deposit goes to him
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_2", &[Coin::new(12345, MOCK_UTOKEN)]),
        ExecuteMsg::Bond {
            receiver: None,
            donate: Some(true),
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::DonationsDisabled {});

    // allow donations
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("owner", &[Coin::new(12345, MOCK_UTOKEN)]),
        ExecuteMsg::UpdateConfig {
            protocol_fee_contract: None,
            protocol_reward_fee: None,
            operator: None,
            stages_preset: None,
            withdrawals_preset: None,
            allow_donations: Some(true),
            vote_operator: None,
            default_max_spread: None,
            epoch_period: None,
            unbond_period: None,
        },
    )
    .unwrap();

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_2", &[Coin::new(12345, MOCK_UTOKEN)]),
        ExecuteMsg::Bond {
            receiver: None,
            donate: Some(true),
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 1);
    assert_eq!(res.messages[0].msg, stake.deposit_msg(Uint128::new(12345)).unwrap());

    deps.querier.set_bank_balances(&[coin(0, MOCK_UTOKEN)]);

    // nothing has been minted -> ustake stays the same, only utoken and exchange rate is changing.
    let res: StateResponse = query_helper(deps.as_ref(), QueryMsg::State {});
    assert_eq!(
        res,
        StateResponse {
            total_ustake: Uint128::new(1000000),
            total_utoken: Uint128::new(1012345),
            exchange_rate: Decimal::from_ratio(1012345u128, 1000000u128),
            unlocked_coins: vec![],
            unbonding: Uint128::zero(),
            available: Uint128::new(0),
            tvl_utoken: Uint128::new(1012345),
        }
    );
}

#[test]
fn harvesting() {
    let (mut deps, mut stake) = setup_test();
    stake.total_supply = Uint128::new(1000000);
    stake.total_utoken_bonded = Uint128::new(1000000);
    State::default().stake_token.save(deps.as_mut().storage, &stake).unwrap();

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("worker", &[]),
        ExecuteMsg::Harvest {
            stages: None,
            withdrawals: None,
            cw20_assets: None,
            native_denoms: None,
            router: None,
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 3);
    assert_eq!(res.messages[0].msg, stake.claim_rewards_msg(&mock_env(), vec![], vec![]).unwrap());
    assert_eq!(res.messages[1], check_received_coin(0, 0));

    assert_eq!(
        res.messages[2],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::Reinvest {})).unwrap(),
            funds: vec![]
        }))
    );
}

#[test]
fn harvesting_with_balance() {
    let (mut deps, mut stake) = setup_test();
    stake.total_supply = Uint128::new(1000000);
    stake.total_utoken_bonded = Uint128::new(1000000);
    State::default().stake_token.save(deps.as_mut().storage, &stake).unwrap();

    deps.querier.set_bank_balances(&[coin(1000, MOCK_UTOKEN), coin(100, get_stake_full_denom())]);

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("worker", &[]),
        ExecuteMsg::Harvest {
            stages: None,
            withdrawals: None,
            cw20_assets: None,
            native_denoms: None,
            router: None,
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 3);
    assert_eq!(res.messages[0].msg, stake.claim_rewards_msg(&mock_env(), vec![], vec![]).unwrap());
    assert_eq!(res.messages[1], check_received_coin(1000, 100));

    assert_eq!(
        res.messages[2],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::Reinvest {})).unwrap(),
            funds: vec![]
        }))
    );
}

#[test]
fn registering_unlocked_coins() {
    let (mut deps, _) = setup_test();
    let state = State::default();
    deps.querier.set_bank_balances(&[coin(100 + 123, MOCK_UTOKEN)]);
    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(MOCK_CONTRACT_ADDR, &[]),
        ExecuteMsg::Callback(CallbackMsg::CheckReceivedCoin {
            snapshot: native_asset_info(MOCK_UTOKEN.to_string()).with_balance(100u128),
            snapshot_stake: native_asset_info(get_stake_full_denom()).with_balance(0u128),
        }),
    )
    .unwrap();
    assert_eq!(
        res.events,
        vec![Event::new("erishub/received")
            .add_attribute("received_coin", 123.to_string() + MOCK_UTOKEN)]
    );
    assert_eq!(res.messages.len(), 0);
    // Unlocked coins in contract state should have been updated
    let unlocked_coins = state.unlocked_coins.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        unlocked_coins,
        vec![native_asset_info(MOCK_UTOKEN.to_string()).with_balance(123u128)]
    );
}

#[test]
fn registering_unlocked_stake_coins() -> StdResult<()> {
    let (mut deps, mut stake) = setup_test();
    let state = State::default();

    stake.total_supply = Uint128::new(1000);
    stake.total_utoken_bonded = Uint128::new(1000);

    state.stake_token.save(deps.as_mut().storage, &stake)?;

    deps.querier
        .set_bank_balances(&[coin(100 + 123, MOCK_UTOKEN), coin(100, get_stake_full_denom())]);

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(MOCK_CONTRACT_ADDR, &[]),
        ExecuteMsg::Callback(CallbackMsg::CheckReceivedCoin {
            snapshot: native_asset_info(MOCK_UTOKEN.to_string()).with_balance(100u128),
            snapshot_stake: native_asset_info(get_stake_full_denom()).with_balance(0u128),
        }),
    )
    .unwrap();
    assert_eq!(
        res.events,
        vec![Event::new("erishub/received")
            .add_attribute("received_coin", 123.to_string() + MOCK_UTOKEN)
            .add_attribute("received_coin", 100.to_string() + &get_stake_full_denom())]
    );

    assert_eq!(res.messages.len(), 0);

    // Unlocked coins in contract state should have been updated
    let unlocked_coins = state.unlocked_coins.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        unlocked_coins,
        vec![
            native_asset_info(MOCK_UTOKEN.to_string()).with_balance(123u128),
            native_asset_info(get_stake_full_denom()).with_balance(100u128)
        ]
    );
    Ok(())
}

#[test]
fn reinvesting() {
    let (mut deps, mut stake) = setup_test();
    let state = State::default();

    stake.total_supply = Uint128::new(1000000);
    stake.total_utoken_bonded = Uint128::new(1000000);

    state.stake_token.save(deps.as_mut().storage, &stake).unwrap();

    // After the swaps, `unlocked_coins` should contain only utoken and unknown denoms
    state
        .unlocked_coins
        .save(
            deps.as_mut().storage,
            &vec![
                native_asset_info(MOCK_UTOKEN.to_string()).with_balance(234u128),
                native_asset_info(get_stake_full_denom()).with_balance(69420u128),
                native_asset_info("unknown".to_string()).with_balance(123u128),
            ],
        )
        .unwrap();

    set_total_stake_supply(&state, &mut deps, 100000);

    // Bob has the smallest amount of delegations, so all proceeds go to him
    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(MOCK_CONTRACT_ADDR, &[]),
        ExecuteMsg::Callback(CallbackMsg::Reinvest {}),
    )
    .unwrap();

    assert_eq!(res.messages.len(), 4);

    let total = Uint128::from(234u128);
    let fee =
        Decimal::from_ratio(1u128, 100u128).checked_mul_uint(total).expect("expects fee result");
    let delegated = total.saturating_sub(fee);

    assert_eq!(res.messages[0].msg, stake.deposit_msg(delegated).unwrap());
    assert_eq!(
        res.messages[1].msg.without_generic(),
        native_asset_info(MOCK_UTOKEN.to_string()).with_balance(fee).into_msg("fee").unwrap()
    );

    let total = Uint128::from(69420u128);
    let fee =
        Decimal::from_ratio(1u128, 100u128).checked_mul_uint(total).expect("expects fee result");
    let burnt = total.saturating_sub(fee);

    assert_eq!(res.messages[2].msg, chain_test().create_burn_msg(get_stake_full_denom(), burnt));
    assert_eq!(
        res.messages[3].msg.without_generic(),
        native_asset_info(get_stake_full_denom()).with_balance(fee).into_msg("fee").unwrap()
    );

    assert_eq!(
        state.stake_token.load(deps.as_ref().storage).unwrap().total_supply,
        Uint128::new(100000 - burnt.u128())
    );

    // Storage should have been updated
    let unlocked_coins = state.unlocked_coins.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        unlocked_coins,
        vec![native_asset_info("unknown".to_string()).with_balance(123u128)]
    );
}

#[test]
fn queuing_unbond() {
    let (mut deps, _) = setup_test();
    let state = State::default();

    // Only Stake token is accepted for unbonding requests
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("random_sender", &[Coin::new(100, "random_token")]),
        ExecuteMsg::QueueUnbond {
            receiver: None,
        },
    )
    .unwrap_err();

    assert_eq!(err, ContractError::ExpectingStakeToken("random_token".into()));

    // User 1 creates an unbonding request before `est_unbond_start_time` is reached. The unbond
    // request is saved, but not the pending batch is not submitted for unbonding
    let res = execute(
        deps.as_mut(),
        mock_env_at_timestamp(12345), // est_unbond_start_time = 269200
        mock_info("user_1", &[Coin::new(23456, get_stake_full_denom())]),
        ExecuteMsg::QueueUnbond {
            receiver: None,
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 0);

    // User 2 creates an unbonding request after `est_unbond_start_time` is reached. The unbond
    // request is saved, and the pending is automatically submitted for unbonding
    let res = execute(
        deps.as_mut(),
        mock_env_at_timestamp(269201), // est_unbond_start_time = 269200
        mock_info("user_2", &[Coin::new(69420, get_stake_full_denom())]),
        ExecuteMsg::QueueUnbond {
            receiver: Some("user_3".to_string()),
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::SubmitBatch {}).unwrap(),
            funds: vec![]
        }))
    );

    // The users' unbonding requests should have been saved
    let ubr1 = state
        .unbond_requests
        .load(deps.as_ref().storage, (1u64, &Addr::unchecked("user_1")))
        .unwrap();
    let ubr2 = state
        .unbond_requests
        .load(deps.as_ref().storage, (1u64, &Addr::unchecked("user_3")))
        .unwrap();

    assert_eq!(
        ubr1,
        UnbondRequest {
            id: 1,
            user: Addr::unchecked("user_1"),
            shares: Uint128::new(23456)
        }
    );
    assert_eq!(
        ubr2,
        UnbondRequest {
            id: 1,
            user: Addr::unchecked("user_3"),
            shares: Uint128::new(69420)
        }
    );

    // Pending batch should have been updated
    let pending_batch = state.pending_batch.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        pending_batch,
        PendingBatch {
            id: 1,
            ustake_to_burn: Uint128::new(92876), // 23,456 + 69,420
            est_unbond_start_time: 269200
        }
    );
}

#[test]
fn submitting_batch() {
    let (mut deps, mut stake) = setup_test();
    let state = State::default();

    // utoken bonded: 1,037,345
    // ustake supply: 1,012,043
    // utoken per ustake: 1.025
    stake.total_utoken_bonded = Uint128::new(1037345);
    stake.total_supply = Uint128::new(1012043);
    state.stake_token.save(deps.as_mut().storage, &stake).unwrap();

    // We continue from the contract state at the end of the last test
    let unbond_requests = vec![
        UnbondRequest {
            id: 1,
            user: Addr::unchecked("user_1"),
            shares: Uint128::new(23456),
        },
        UnbondRequest {
            id: 1,
            user: Addr::unchecked("user_3"),
            shares: Uint128::new(69420),
        },
    ];

    for unbond_request in &unbond_requests {
        state
            .unbond_requests
            .save(
                deps.as_mut().storage,
                (unbond_request.id, &Addr::unchecked(unbond_request.user.clone())),
                unbond_request,
            )
            .unwrap();
    }

    state
        .pending_batch
        .save(
            deps.as_mut().storage,
            &PendingBatch {
                id: 1,
                ustake_to_burn: Uint128::new(92876), // 23,456 + 69,420
                est_unbond_start_time: 269200,
            },
        )
        .unwrap();

    // Anyone can invoke `submit_batch`. Here we continue from the previous test and assume it is
    // invoked automatically as user 2 submits the unbonding request
    //
    // ustake to burn: 23,456 + 69,420 = 92,876
    // utoken to unbond: 1,037,345 * 92,876 / 1,012,043 = 95,197
    //
    // Target: (1,037,345 - 95,197) / 3 = 314,049
    // Remainer: 1
    // Alice:   345,782 - (314,049 + 1) = 31,732
    // Bob:     345,782 - (314,049 + 0) = 31,733
    // Charlie: 345,781 - (314,049 + 0) = 31,732
    let res = execute(
        deps.as_mut(),
        mock_env_at_timestamp(269201),
        mock_info(MOCK_CONTRACT_ADDR, &[]),
        ExecuteMsg::SubmitBatch {},
    )
    .unwrap();

    assert_eq!(res.messages.len(), 2);

    assert_eq!(res.messages[0].msg, stake.unbond_msg(Uint128::new(95197)).unwrap());

    assert_eq!(
        res.messages[1].msg,
        chain_test().create_burn_msg(get_stake_full_denom(), Uint128::new(92876))
    );

    // A new pending batch should have been created
    let pending_batch = state.pending_batch.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        pending_batch,
        PendingBatch {
            id: 2,
            ustake_to_burn: Uint128::zero(),
            est_unbond_start_time: 528401 // 269,201 + 259,200
        }
    );

    // Previous batch should have been updated
    let previous_batch = state.previous_batches.load(deps.as_ref().storage, 1u64).unwrap();
    assert_eq!(
        previous_batch,
        Batch {
            id: 1,
            reconciled: false,
            total_shares: Uint128::new(92876),
            utoken_unclaimed: Uint128::new(95197),
            est_unbond_end_time: 2083601 // 269,201 + 1,814,400
        }
    );

    let res: StateResponse = query_helper_env(deps.as_ref(), QueryMsg::State {}, 2083600);

    let total_ustake = Uint128::from(1012043u128).sub(Uint128::new(23456)).sub(Uint128::new(69420));
    assert_eq!(
        res,
        StateResponse {
            total_ustake,
            total_utoken: Uint128::from(1037345u128),
            exchange_rate: Decimal::from_ratio(1037345u128, total_ustake.u128()),
            unlocked_coins: vec![],
            unbonding: Uint128::from(95197u128),
            available: Uint128::zero(),
            tvl_utoken: Uint128::from(95197u128 + 1037345u128),
        },
    );
}

#[test]
fn reconciling() {
    let (mut deps, stake) = setup_test();
    let state = State::default();

    let previous_batches = vec![
        Batch {
            id: 1,
            reconciled: true,
            total_shares: Uint128::new(92876),
            utoken_unclaimed: Uint128::new(95197), // 1.025 Token per Stake
            est_unbond_end_time: 10000,
        },
        Batch {
            id: 2,
            reconciled: false,
            total_shares: Uint128::new(1345),
            utoken_unclaimed: Uint128::new(1385), // 1.030 Token per Stake
            est_unbond_end_time: 20000,
        },
        Batch {
            id: 3,
            reconciled: false,
            total_shares: Uint128::new(1456),
            utoken_unclaimed: Uint128::new(1506), // 1.035 Token per Stake
            est_unbond_end_time: 30000,
        },
        Batch {
            id: 4,
            reconciled: false,
            total_shares: Uint128::new(1567),
            utoken_unclaimed: Uint128::new(1629), // 1.040 Token per Stake
            est_unbond_end_time: 40000,           // not yet finished unbonding, ignored
        },
    ];

    for previous_batch in &previous_batches {
        state
            .previous_batches
            .save(deps.as_mut().storage, previous_batch.id, previous_batch)
            .unwrap();
    }

    state
        .unlocked_coins
        .save(
            deps.as_mut().storage,
            &vec![
                native_asset_info(MOCK_UTOKEN.to_string()).with_balance(10000u128),
                native_asset_info("ukrw".to_string()).with_balance(234u128),
                native_asset_info("uusd".to_string()).with_balance(345u128),
                native_asset_info(
                    "ibc/0471F1C4E7AFD3F07702BEF6DC365268D64570F7C1FDC98EA6098DD6DE59817B"
                        .to_string(),
                )
                .with_balance(69420u128),
            ],
        )
        .unwrap();

    deps.querier.set_bank_balances(&[
        Coin::new(10000, MOCK_UTOKEN),
        Coin::new(234, "ukrw"),
        Coin::new(345, "uusd"),
        Coin::new(69420, "ibc/0471F1C4E7AFD3F07702BEF6DC365268D64570F7C1FDC98EA6098DD6DE59817B"),
    ]);

    let res = execute(
        deps.as_mut(),
        mock_env_at_timestamp(35000),
        mock_info("worker", &[]),
        ExecuteMsg::Reconcile {},
    )
    .unwrap();

    let assert_balance_msg = ExecuteMsg::Callback(CallbackMsg::AssertBalance {
        expected: native_asset_info(MOCK_UTOKEN.to_string()).with_balance(12891u128),
    });

    assert_eq!(res.messages.len(), 2);
    assert_eq!(res.messages[0].msg, stake.claim_unbonded_msg().unwrap());
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&assert_balance_msg).unwrap(),
            funds: vec![]
        }))
    );

    // Expected received: batch 2 + batch 3 = 1385 + 1506 = 2891
    // Expected unlocked: 10000
    // Expected: 12891
    let batch = state.previous_batches.load(deps.as_ref().storage, 2u64).unwrap();
    assert_eq!(
        batch,
        Batch {
            id: 2,
            reconciled: true,
            total_shares: Uint128::new(1345),
            utoken_unclaimed: Uint128::new(1385),
            est_unbond_end_time: 20000,
        }
    );

    let batch = state.previous_batches.load(deps.as_ref().storage, 3u64).unwrap();
    assert_eq!(
        batch,
        Batch {
            id: 3,
            reconciled: true,
            total_shares: Uint128::new(1456),
            utoken_unclaimed: Uint128::new(1506),
            est_unbond_end_time: 30000,
        }
    );

    // Batches 1 and 4 should not have changed
    let batch = state.previous_batches.load(deps.as_ref().storage, 1u64).unwrap();
    assert_eq!(batch, previous_batches[0]);

    let batch = state.previous_batches.load(deps.as_ref().storage, 4u64).unwrap();
    assert_eq!(batch, previous_batches[3]);

    deps.querier.set_bank_balances(&[
        Coin::new(12345, MOCK_UTOKEN),
        Coin::new(234, "ukrw"),
        Coin::new(345, "uusd"),
        Coin::new(69420, "ibc/0471F1C4E7AFD3F07702BEF6DC365268D64570F7C1FDC98EA6098DD6DE59817B"),
    ]);

    execute(
        deps.as_mut(),
        mock_env_at_timestamp(35000),
        mock_info("worker", &[]),
        assert_balance_msg.clone(),
    )
    .expect_err("Should be self called");

    execute(
        deps.as_mut(),
        mock_env_at_timestamp(35000),
        mock_info(MOCK_CONTRACT_ADDR, &[]),
        assert_balance_msg.clone(),
    )
    .expect_err("Not enough received");

    deps.querier.set_bank_balances(&[
        Coin::new(12891, MOCK_UTOKEN),
        Coin::new(234, "ukrw"),
        Coin::new(345, "uusd"),
        Coin::new(69420, "ibc/0471F1C4E7AFD3F07702BEF6DC365268D64570F7C1FDC98EA6098DD6DE59817B"),
    ]);

    execute(
        deps.as_mut(),
        mock_env_at_timestamp(35000),
        mock_info(MOCK_CONTRACT_ADDR, &[]),
        assert_balance_msg,
    )
    .expect("Should succeed");
}

#[test]
fn reconciling_even_when_everything_ok() {
    let (mut deps, _) = setup_test();
    let state = State::default();

    let previous_batches = vec![
        Batch {
            id: 1,
            reconciled: true,
            total_shares: Uint128::new(100000),
            utoken_unclaimed: Uint128::new(100000),
            est_unbond_end_time: 10000,
        },
        Batch {
            id: 2,
            reconciled: false,
            total_shares: Uint128::new(1000),
            utoken_unclaimed: Uint128::new(1000),
            est_unbond_end_time: 20000,
        },
        Batch {
            id: 3,
            reconciled: false,
            total_shares: Uint128::new(1500),
            utoken_unclaimed: Uint128::new(1500),
            est_unbond_end_time: 30000,
        },
        Batch {
            id: 4,
            reconciled: false,
            total_shares: Uint128::new(1500),
            utoken_unclaimed: Uint128::new(1500),
            est_unbond_end_time: 40000, // not yet finished unbonding, ignored
        },
    ];

    for previous_batch in &previous_batches {
        state
            .previous_batches
            .save(deps.as_mut().storage, previous_batch.id, previous_batch)
            .unwrap();
    }

    state
        .unlocked_coins
        .save(
            deps.as_mut().storage,
            &vec![native_asset_info(MOCK_UTOKEN.to_string()).with_balance(1000u128)],
        )
        .unwrap();

    deps.querier.set_bank_balances(&[Coin::new(3500, MOCK_UTOKEN)]);

    execute(
        deps.as_mut(),
        mock_env_at_timestamp(35000),
        mock_info("worker", &[]),
        ExecuteMsg::Reconcile {},
    )
    .unwrap();

    let batch = state.previous_batches.load(deps.as_ref().storage, 2u64).unwrap();
    assert_eq!(
        batch,
        Batch {
            id: 2,
            reconciled: true,
            total_shares: Uint128::new(1000),
            utoken_unclaimed: Uint128::new(1000),
            est_unbond_end_time: 20000,
        }
    );

    let batch = state.previous_batches.load(deps.as_ref().storage, 3u64).unwrap();
    assert_eq!(
        batch,
        Batch {
            id: 3,
            reconciled: true,
            total_shares: Uint128::new(1500),
            utoken_unclaimed: Uint128::new(1500),
            est_unbond_end_time: 30000,
        }
    );

    // Batches 1 and 4 should not have changed
    let batch = state.previous_batches.load(deps.as_ref().storage, 1u64).unwrap();
    assert_eq!(batch, previous_batches[0]);

    let batch = state.previous_batches.load(deps.as_ref().storage, 4u64).unwrap();
    assert_eq!(batch, previous_batches[3]);
}

#[test]
fn reconciling_underflow() {
    let (mut deps, _) = setup_test();
    let state = State::default();
    let previous_batches = vec![
        Batch {
            id: 1,
            reconciled: true,
            total_shares: Uint128::new(92876),
            utoken_unclaimed: Uint128::new(95197), // 1.025 Token per Stake
            est_unbond_end_time: 10000,
        },
        Batch {
            id: 2,
            reconciled: false,
            total_shares: Uint128::new(1345),
            utoken_unclaimed: Uint128::new(1385), // 1.030 Token per Stake
            est_unbond_end_time: 20000,
        },
        Batch {
            id: 3,
            reconciled: false,
            total_shares: Uint128::new(1456),
            utoken_unclaimed: Uint128::new(1506), // 1.035 Token per Stake
            est_unbond_end_time: 30000,
        },
        Batch {
            id: 4,
            reconciled: false,
            total_shares: Uint128::new(1),
            utoken_unclaimed: Uint128::new(1),
            est_unbond_end_time: 30001,
        },
    ];
    for previous_batch in &previous_batches {
        state
            .previous_batches
            .save(deps.as_mut().storage, previous_batch.id, previous_batch)
            .unwrap();
    }
    state
        .unlocked_coins
        .save(
            deps.as_mut().storage,
            &vec![
                native_asset_info(MOCK_UTOKEN.to_string()).with_balance(10000u128),
                native_asset_info("ukrw".to_string()).with_balance(234u128),
                native_asset_info("uusd".to_string()).with_balance(345u128),
                native_asset_info(
                    "ibc/0471F1C4E7AFD3F07702BEF6DC365268D64570F7C1FDC98EA6098DD6DE59817B"
                        .to_string(),
                )
                .with_balance(69420u128),
            ],
        )
        .unwrap();
    deps.querier.set_bank_balances(&[
        Coin::new(12345, MOCK_UTOKEN),
        Coin::new(234, "ukrw"),
        Coin::new(345, "uusd"),
        Coin::new(69420, "ibc/0471F1C4E7AFD3F07702BEF6DC365268D64570F7C1FDC98EA6098DD6DE59817B"),
    ]);
    execute(
        deps.as_mut(),
        mock_env_at_timestamp(35000),
        mock_info("worker", &[]),
        ExecuteMsg::Reconcile {},
    )
    .unwrap();
}

#[test]
fn reconciling_underflow_second() {
    let (mut deps, _) = setup_test();
    let state = State::default();
    let previous_batches = vec![
        Batch {
            id: 1,
            reconciled: true,
            total_shares: Uint128::new(92876),
            utoken_unclaimed: Uint128::new(95197), // 1.025 Token per Stake
            est_unbond_end_time: 10000,
        },
        Batch {
            id: 2,
            reconciled: false,
            total_shares: Uint128::new(1345),
            utoken_unclaimed: Uint128::new(1385), // 1.030 Token per Stake
            est_unbond_end_time: 20000,
        },
        Batch {
            id: 3,
            reconciled: false,
            total_shares: Uint128::new(176),
            utoken_unclaimed: Uint128::new(183), // 1.035 Token per Stake
            est_unbond_end_time: 30000,
        },
        Batch {
            id: 4,
            reconciled: false,
            total_shares: Uint128::new(1),
            utoken_unclaimed: Uint128::new(1),
            est_unbond_end_time: 30001,
        },
    ];
    for previous_batch in &previous_batches {
        state
            .previous_batches
            .save(deps.as_mut().storage, previous_batch.id, previous_batch)
            .unwrap();
    }
    state
        .unlocked_coins
        .save(
            deps.as_mut().storage,
            &vec![
                native_asset_info(MOCK_UTOKEN.to_string()).with_balance(10000u128),
                native_asset_info("ukrw".to_string()).with_balance(234u128),
                native_asset_info("uusd".to_string()).with_balance(345u128),
                native_asset_info(
                    "ibc/0471F1C4E7AFD3F07702BEF6DC365268D64570F7C1FDC98EA6098DD6DE59817B"
                        .to_string(),
                )
                .with_balance(69420u128),
            ],
        )
        .unwrap();
    deps.querier.set_bank_balances(&[
        Coin::new(12345 - 1323, MOCK_UTOKEN),
        Coin::new(234, "ukrw"),
        Coin::new(345, "uusd"),
        Coin::new(69420, "ibc/0471F1C4E7AFD3F07702BEF6DC365268D64570F7C1FDC98EA6098DD6DE59817B"),
    ]);
    execute(
        deps.as_mut(),
        mock_env_at_timestamp(35000),
        mock_info("worker", &[]),
        ExecuteMsg::Reconcile {},
    )
    .unwrap();
}

#[test]
fn withdrawing_unbonded() {
    let (mut deps, _) = setup_test();
    let state = State::default();

    // We simulate a most general case:
    // - batches 1 and 2 have finished unbonding
    // - batch 3 have been submitted for unbonding but have not finished
    // - batch 4 is still pending
    let unbond_requests = vec![
        UnbondRequest {
            id: 1,
            user: Addr::unchecked("user_1"),
            shares: Uint128::new(23456),
        },
        UnbondRequest {
            id: 1,
            user: Addr::unchecked("user_3"),
            shares: Uint128::new(69420),
        },
        UnbondRequest {
            id: 2,
            user: Addr::unchecked("user_1"),
            shares: Uint128::new(34567),
        },
        UnbondRequest {
            id: 3,
            user: Addr::unchecked("user_1"),
            shares: Uint128::new(45678),
        },
        UnbondRequest {
            id: 4,
            user: Addr::unchecked("user_1"),
            shares: Uint128::new(56789),
        },
    ];

    for unbond_request in &unbond_requests {
        state
            .unbond_requests
            .save(
                deps.as_mut().storage,
                (unbond_request.id, &Addr::unchecked(unbond_request.user.clone())),
                unbond_request,
            )
            .unwrap();
    }

    let previous_batches = vec![
        Batch {
            id: 1,
            reconciled: true,
            total_shares: Uint128::new(92876),
            utoken_unclaimed: Uint128::new(95197), // 1.025 Token per Stake
            est_unbond_end_time: 10000,
        },
        Batch {
            id: 2,
            reconciled: true,
            total_shares: Uint128::new(34567),
            utoken_unclaimed: Uint128::new(35604), // 1.030 Token per Stake
            est_unbond_end_time: 20000,
        },
        Batch {
            id: 3,
            reconciled: false, // finished unbonding, but not reconciled; ignored
            total_shares: Uint128::new(45678),
            utoken_unclaimed: Uint128::new(47276), // 1.035 Token per Stake
            est_unbond_end_time: 20000,
        },
        Batch {
            id: 4,
            reconciled: true,
            total_shares: Uint128::new(56789),
            utoken_unclaimed: Uint128::new(59060), // 1.040 Token per Stake
            est_unbond_end_time: 30000, // reconciled, but not yet finished unbonding; ignored
        },
    ];

    for previous_batch in &previous_batches {
        state
            .previous_batches
            .save(deps.as_mut().storage, previous_batch.id, previous_batch)
            .unwrap();
    }

    state
        .pending_batch
        .save(
            deps.as_mut().storage,
            &PendingBatch {
                id: 4,
                ustake_to_burn: Uint128::new(56789),
                est_unbond_start_time: 100000,
            },
        )
        .unwrap();

    // Attempt to withdraw before any batch has completed unbonding. Should error
    let err = execute(
        deps.as_mut(),
        mock_env_at_timestamp(5000),
        mock_info("user_1", &[]),
        ExecuteMsg::WithdrawUnbonded {
            receiver: None,
        },
    )
    .unwrap_err();

    assert_eq!(err, ContractError::CantBeZero("withdrawable amount".into()));

    // Attempt to withdraw once batches 1 and 2 have finished unbonding, but 3 has not yet
    //
    // Withdrawable from batch 1: 95,197 * 23,456 / 92,876 = 24,042
    // Withdrawable from batch 2: 35,604
    // Total withdrawable: 24,042 + 35,604 = 59,646
    //
    // Batch 1 should be updated:
    // Total shares: 92,876 - 23,456 = 69,420
    // Unclaimed utoken: 95,197 - 24,042 = 71,155
    //
    // Batch 2 is completely withdrawn, should be purged from storage
    let res = execute(
        deps.as_mut(),
        mock_env_at_timestamp(25000),
        mock_info("user_1", &[]),
        ExecuteMsg::WithdrawUnbonded {
            receiver: None,
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: "user_1".to_string(),
            amount: vec![Coin::new(59646, MOCK_UTOKEN)]
        }))
    );

    // Previous batches should have been updated
    let batch = state.previous_batches.load(deps.as_ref().storage, 1u64).unwrap();
    assert_eq!(
        batch,
        Batch {
            id: 1,
            reconciled: true,
            total_shares: Uint128::new(69420),
            utoken_unclaimed: Uint128::new(71155),
            est_unbond_end_time: 10000,
        }
    );

    let err = state.previous_batches.load(deps.as_ref().storage, 2u64).unwrap_err();
    assert_eq!(
        err,
        StdError::NotFound {
            kind: "eris::hub::Batch".to_string()
        }
    );

    // User 1's unbond requests in batches 1 and 2 should have been deleted
    let err1 = state
        .unbond_requests
        .load(deps.as_ref().storage, (1u64, &Addr::unchecked("user_1")))
        .unwrap_err();
    let err2 = state
        .unbond_requests
        .load(deps.as_ref().storage, (1u64, &Addr::unchecked("user_1")))
        .unwrap_err();

    assert_eq!(
        err1,
        StdError::NotFound {
            kind: "eris::hub::UnbondRequest".to_string()
        }
    );
    assert_eq!(
        err2,
        StdError::NotFound {
            kind: "eris::hub::UnbondRequest".to_string()
        }
    );

    // User 3 attempt to withdraw; also specifying a receiver
    let res = execute(
        deps.as_mut(),
        mock_env_at_timestamp(25000),
        mock_info("user_3", &[]),
        ExecuteMsg::WithdrawUnbonded {
            receiver: Some("user_2".to_string()),
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: "user_2".to_string(),
            amount: vec![Coin::new(71155, MOCK_UTOKEN)]
        }))
    );

    // Batch 1 and user 2's unbonding request should have been purged from storage
    let err = state.previous_batches.load(deps.as_ref().storage, 1u64).unwrap_err();
    assert_eq!(
        err,
        StdError::NotFound {
            kind: "eris::hub::Batch".to_string()
        }
    );

    let err = state
        .unbond_requests
        .load(deps.as_ref().storage, (1u64, &Addr::unchecked("user_3")))
        .unwrap_err();

    assert_eq!(
        err,
        StdError::NotFound {
            kind: "eris::hub::UnbondRequest".to_string()
        }
    );
}

#[test]
fn transferring_ownership() {
    let (mut deps, _) = setup_test();
    let state = State::default();

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("jake", &[]),
        ExecuteMsg::TransferOwnership {
            new_owner: "jake".to_string(),
        },
    )
    .unwrap_err();

    assert_eq!(err, ContractError::Unauthorized {});

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("owner", &[]),
        ExecuteMsg::TransferOwnership {
            new_owner: "jake".to_string(),
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 0);

    let owner = state.owner.load(deps.as_ref().storage).unwrap();
    assert_eq!(owner, Addr::unchecked("owner"));

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("pumpkin", &[]),
        ExecuteMsg::AcceptOwnership {},
    )
    .unwrap_err();

    assert_eq!(err, ContractError::UnauthorizedSenderNotNewOwner {});

    let res =
        execute(deps.as_mut(), mock_env(), mock_info("jake", &[]), ExecuteMsg::AcceptOwnership {})
            .unwrap();

    assert_eq!(res.messages.len(), 0);

    let owner = state.owner.load(deps.as_ref().storage).unwrap();
    assert_eq!(owner, Addr::unchecked("jake"));
}

//--------------------------------------------------------------------------------------------------
// Fee Config
//--------------------------------------------------------------------------------------------------

#[test]
fn update_fee() {
    let (mut deps, _) = setup_test();
    let state = State::default();

    let config = state.fee_config.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        config,
        FeeConfig {
            protocol_fee_contract: Addr::unchecked("fee"),
            protocol_reward_fee: Decimal::from_ratio(1u128, 100u128)
        }
    );

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("jake", &[]),
        ExecuteMsg::UpdateConfig {
            protocol_fee_contract: None,
            protocol_reward_fee: Some(Decimal::from_ratio(11u128, 100u128)),
            operator: None,
            stages_preset: None,
            withdrawals_preset: None,
            allow_donations: None,
            vote_operator: None,
            default_max_spread: None,
            epoch_period: None,
            unbond_period: None,
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("owner", &[]),
        ExecuteMsg::UpdateConfig {
            protocol_fee_contract: None,
            protocol_reward_fee: Some(Decimal::from_ratio(11u128, 100u128)),
            operator: None,
            stages_preset: None,
            withdrawals_preset: None,
            allow_donations: None,
            vote_operator: None,
            default_max_spread: None,
            epoch_period: None,
            unbond_period: None,
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::ProtocolRewardFeeTooHigh {});

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("owner", &[]),
        ExecuteMsg::UpdateConfig {
            protocol_fee_contract: Some("fee-new".to_string()),
            protocol_reward_fee: Some(Decimal::from_ratio(10u128, 100u128)),
            operator: None,
            stages_preset: None,
            withdrawals_preset: None,
            allow_donations: None,
            vote_operator: None,
            default_max_spread: None,
            epoch_period: None,
            unbond_period: None,
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 0);

    let config = state.fee_config.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        config,
        FeeConfig {
            protocol_fee_contract: Addr::unchecked("fee-new"),
            protocol_reward_fee: Decimal::from_ratio(10u128, 100u128)
        }
    );
}

//--------------------------------------------------------------------------------------------------
// Gov
//--------------------------------------------------------------------------------------------------

#[test]
fn vote() {
    let (mut deps, stake) = setup_test();
    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("jake", &[]),
        ExecuteMsg::Vote {
            proposal_id: 3,
            vote: cosmwasm_std::VoteOption::Yes,
        },
    )
    .unwrap_err();
    assert_eq!(res, ContractError::NoVoteOperatorSet {});

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("owner", &[]),
        ExecuteMsg::UpdateConfig {
            protocol_fee_contract: None,
            protocol_reward_fee: None,
            allow_donations: None,
            vote_operator: Some("vote_operator".to_string()),
            operator: None,
            stages_preset: None,
            withdrawals_preset: None,
            default_max_spread: None,
            epoch_period: None,
            unbond_period: None,
        },
    )
    .unwrap();

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("jake", &[]),
        ExecuteMsg::Vote {
            proposal_id: 3,
            vote: cosmwasm_std::VoteOption::Yes,
        },
    )
    .unwrap_err();
    assert_eq!(res, ContractError::UnauthorizedSenderNotVoteOperator {});

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("vote_operator", &[]),
        ExecuteMsg::Vote {
            proposal_id: 3,
            vote: cosmwasm_std::VoteOption::Yes,
        },
    )
    .unwrap();
    assert_eq!(res.messages.len(), 1);

    assert_eq!(res.messages[0].msg, stake.vote_msg(3, VoteOption::Yes).unwrap());
}

//--------------------------------------------------------------------------------------------------
// Queries
//--------------------------------------------------------------------------------------------------

#[test]
fn querying_previous_batches() {
    let mut deps = mock_dependencies();

    let batches = vec![
        Batch {
            id: 1,
            reconciled: false,
            total_shares: Uint128::new(123),
            utoken_unclaimed: Uint128::new(678),
            est_unbond_end_time: 10000,
        },
        Batch {
            id: 2,
            reconciled: true,
            total_shares: Uint128::new(234),
            utoken_unclaimed: Uint128::new(789),
            est_unbond_end_time: 15000,
        },
        Batch {
            id: 3,
            reconciled: false,
            total_shares: Uint128::new(345),
            utoken_unclaimed: Uint128::new(890),
            est_unbond_end_time: 20000,
        },
        Batch {
            id: 4,
            reconciled: true,
            total_shares: Uint128::new(456),
            utoken_unclaimed: Uint128::new(999),
            est_unbond_end_time: 25000,
        },
    ];

    let state = State::default();
    for batch in &batches {
        state.previous_batches.save(deps.as_mut().storage, batch.id, batch).unwrap();
    }

    // Querying a single batch
    let res: Batch = query_helper(deps.as_ref(), QueryMsg::PreviousBatch(1));
    assert_eq!(res, batches[0].clone());

    let res: Batch = query_helper(deps.as_ref(), QueryMsg::PreviousBatch(2));
    assert_eq!(res, batches[1].clone());

    // Query multiple batches
    let res: Vec<Batch> = query_helper(
        deps.as_ref(),
        QueryMsg::PreviousBatches {
            start_after: None,
            limit: None,
        },
    );
    assert_eq!(res, batches);

    let res: Vec<Batch> = query_helper(
        deps.as_ref(),
        QueryMsg::PreviousBatches {
            start_after: Some(1),
            limit: None,
        },
    );
    assert_eq!(res, vec![batches[1].clone(), batches[2].clone(), batches[3].clone()]);

    let res: Vec<Batch> = query_helper(
        deps.as_ref(),
        QueryMsg::PreviousBatches {
            start_after: Some(4),
            limit: None,
        },
    );
    assert_eq!(res, vec![]);

    // Query multiple batches, indexed by whether it has been reconciled
    let res = state
        .previous_batches
        .idx
        .reconciled
        .prefix(true.into())
        .range(deps.as_ref().storage, None, None, Order::Ascending)
        .map(|item| {
            let (_, v) = item.unwrap();
            v
        })
        .collect::<Vec<_>>();

    assert_eq!(res, vec![batches[1].clone(), batches[3].clone()]);

    let res = state
        .previous_batches
        .idx
        .reconciled
        .prefix(false.into())
        .range(deps.as_ref().storage, None, None, Order::Ascending)
        .map(|item| {
            let (_, v) = item.unwrap();
            v
        })
        .collect::<Vec<_>>();

    assert_eq!(res, vec![batches[0].clone(), batches[2].clone()]);
}

#[test]
fn querying_unbond_requests() {
    let mut deps = mock_dependencies();
    let state = State::default();

    let unbond_requests = vec![
        UnbondRequest {
            id: 1,
            user: Addr::unchecked("alice"),
            shares: Uint128::new(123),
        },
        UnbondRequest {
            id: 1,
            user: Addr::unchecked("bob"),
            shares: Uint128::new(234),
        },
        UnbondRequest {
            id: 1,
            user: Addr::unchecked("charlie"),
            shares: Uint128::new(345),
        },
        UnbondRequest {
            id: 2,
            user: Addr::unchecked("alice"),
            shares: Uint128::new(456),
        },
    ];

    for unbond_request in &unbond_requests {
        state
            .unbond_requests
            .save(
                deps.as_mut().storage,
                (unbond_request.id, &Addr::unchecked(unbond_request.user.clone())),
                unbond_request,
            )
            .unwrap();
    }

    let res: Vec<UnbondRequestsByBatchResponseItem> = query_helper(
        deps.as_ref(),
        QueryMsg::UnbondRequestsByBatch {
            id: 1,
            start_after: None,
            limit: None,
        },
    );
    assert_eq!(
        res,
        vec![
            unbond_requests[0].clone().into(),
            unbond_requests[1].clone().into(),
            unbond_requests[2].clone().into(),
        ]
    );

    let res: Vec<UnbondRequestsByBatchResponseItem> = query_helper(
        deps.as_ref(),
        QueryMsg::UnbondRequestsByBatch {
            id: 2,
            start_after: None,
            limit: None,
        },
    );
    assert_eq!(res, vec![unbond_requests[3].clone().into()]);

    let res: Vec<UnbondRequestsByUserResponseItem> = query_helper(
        deps.as_ref(),
        QueryMsg::UnbondRequestsByUser {
            user: "alice".to_string(),
            start_after: None,
            limit: None,
        },
    );
    assert_eq!(res, vec![unbond_requests[0].clone().into(), unbond_requests[3].clone().into(),]);

    let res: Vec<UnbondRequestsByUserResponseItem> = query_helper(
        deps.as_ref(),
        QueryMsg::UnbondRequestsByUser {
            user: "alice".to_string(),
            start_after: Some(1u64),
            limit: None,
        },
    );
    assert_eq!(res, vec![unbond_requests[3].clone().into()]);
}

#[test]
fn querying_unbond_requests_details() {
    let mut deps = mock_dependencies();
    let state = State::default();

    let unbond_requests = vec![
        UnbondRequest {
            id: 1,
            user: Addr::unchecked("alice"),
            shares: Uint128::new(123),
        },
        UnbondRequest {
            id: 1,
            user: Addr::unchecked("bob"),
            shares: Uint128::new(234),
        },
        UnbondRequest {
            id: 1,
            user: Addr::unchecked("charlie"),
            shares: Uint128::new(345),
        },
        UnbondRequest {
            id: 2,
            user: Addr::unchecked("alice"),
            shares: Uint128::new(456),
        },
        UnbondRequest {
            id: 3,
            user: Addr::unchecked("alice"),
            shares: Uint128::new(555),
        },
    ];

    let pending = PendingBatch {
        id: 3,
        ustake_to_burn: Uint128::new(1000),
        est_unbond_start_time: 20000,
    };

    state.pending_batch.save(deps.as_mut().storage, &pending).unwrap();

    let batches = vec![
        Batch {
            id: 1,
            reconciled: false,
            total_shares: Uint128::new(123),
            utoken_unclaimed: Uint128::new(678),
            est_unbond_end_time: 10000,
        },
        Batch {
            id: 2,
            reconciled: false,
            total_shares: Uint128::new(234),
            utoken_unclaimed: Uint128::new(789),
            est_unbond_end_time: 15000,
        },
    ];

    for batch in &batches {
        state.previous_batches.save(deps.as_mut().storage, batch.id, batch).unwrap();
    }

    for unbond_request in &unbond_requests {
        state
            .unbond_requests
            .save(
                deps.as_mut().storage,
                (unbond_request.id, &Addr::unchecked(unbond_request.user.clone())),
                unbond_request,
            )
            .unwrap();
    }

    let res: Vec<UnbondRequestsByUserResponseItemDetails> = query_helper_env(
        deps.as_ref(),
        QueryMsg::UnbondRequestsByUserDetails {
            user: "alice".to_string(),
            start_after: None,
            limit: None,
        },
        12000,
    );
    assert_eq!(
        res,
        vec![
            UnbondRequestsByUserResponseItemDetails {
                id: 1,
                shares: Uint128::new(123),
                state: "COMPLETED".to_string(),
                batch: Some(batches[0].clone()),
                pending: None
            },
            UnbondRequestsByUserResponseItemDetails {
                id: 2,
                shares: Uint128::new(456),
                state: "UNBONDING".to_string(),
                batch: Some(batches[1].clone()),
                pending: None
            },
            UnbondRequestsByUserResponseItemDetails {
                id: 3,
                shares: Uint128::new(555),
                state: "PENDING".to_string(),
                batch: None,
                pending: Some(pending)
            }
        ]
    );
}

//--------------------------------------------------------------------------------------------------
// Coins
//--------------------------------------------------------------------------------------------------

#[test]
fn receiving_funds() {
    let err = validate_received_funds(&[], &mock_utoken()).unwrap_err();
    assert_eq!(err, StdError::generic_err("must deposit exactly one coin; received 0"));

    let err = validate_received_funds(
        &[Coin::new(12345, "uatom"), Coin::new(23456, MOCK_UTOKEN)],
        &mock_utoken(),
    )
    .unwrap_err();
    assert_eq!(err, StdError::generic_err("must deposit exactly one coin; received 2"));

    let err = validate_received_funds(&[Coin::new(12345, "uatom")], &mock_utoken()).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err(format!("expected {} deposit, received uatom", MOCK_UTOKEN))
    );

    let err = validate_received_funds(&[Coin::new(0, MOCK_UTOKEN)], &mock_utoken()).unwrap_err();
    assert_eq!(err, StdError::generic_err("deposit amount must be non-zero"));

    let amount = validate_received_funds(&[Coin::new(69420, MOCK_UTOKEN)], &mock_utoken()).unwrap();
    assert_eq!(amount, Uint128::new(69420));
}
