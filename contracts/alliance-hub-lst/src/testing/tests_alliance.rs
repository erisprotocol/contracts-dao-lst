use astroport::asset::{native_asset, native_asset_info, AssetInfoExt};
use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    coin, to_json_binary, Addr, Coin, CosmosMsg, Decimal, Event, StdError, StdResult, SubMsg,
    Uint128, WasmMsg,
};
use eris::DecimalCheckedOps;

use eris::adapters::asset::AssetEx;
use eris::helper::validate_received_funds;
use eris::hub_alliance::{
    CallbackMsg, ConfigResponse, ExecuteMsg, FeeConfig, QueryMsg, StakeToken, StateResponse,
};

use eris_chain_shared::chain_trait::ChainInterface;

use crate::contract::execute;
use crate::error::ContractError;
use crate::state::State;
use crate::testing::helpers::{
    chain_test, check_received_coin, get_stake_full_denom, mock_utoken, set_total_stake_supply,
    setup_test, MOCK_UTOKEN,
};
use crate::testing::WithoutGeneric;

use super::helpers::{mock_env_at_timestamp, query_helper};

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
            fee_config: FeeConfig {
                protocol_fee_contract: Addr::unchecked("fee"),
                protocol_reward_fee: Decimal::from_ratio(1u128, 100u128)
            },

            operator: "operator".to_string(),
            stages_preset: vec![],
            withdrawals_preset: vec![],
            allow_donations: false,
            utoken: native_asset_info(MOCK_UTOKEN.to_string()),
            dao_interface: eris::hub::DaoInterface::Alliance {
                addr: Addr::unchecked("alliance")
            },
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
            available: Uint128::zero(),
            tvl_utoken: Uint128::zero(),
        },
    );
}

#[test]
fn bond() {
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
    assert_eq!(
        res.messages[index].msg,
        stake
            .dao_interface
            .deposit_msg(&stake.utoken, Uint128::new(1000000), MOCK_CONTRACT_ADDR.to_string())
            .unwrap()
    );

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
    assert_eq!(
        res.messages[index].msg,
        stake
            .dao_interface
            .deposit_msg(&stake.utoken, Uint128::new(12345), MOCK_CONTRACT_ADDR.to_string())
            .unwrap()
    );

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
            available: Uint128::new(0),
            tvl_utoken: Uint128::new(1012345),
        }
    );
}

#[test]
fn swap_bond() {
    let (mut deps, stake) = setup_test();

    deps.querier.set_bank_balances(&[coin(1000000, MOCK_UTOKEN)]);

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_1", &[Coin::new(1000000, MOCK_UTOKEN)]),
        ExecuteMsg::Swap {
            offer_asset: native_asset(MOCK_UTOKEN.to_string(), Uint128::new(1000001)),
            belief_price: None,
            max_spread: None,
            to: None,
        },
    )
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        "Generic error: Native token balance mismatch between the argument and the transferred"
            .to_string()
    );

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_1", &[Coin::new(1000000, MOCK_UTOKEN)]),
        ExecuteMsg::Swap {
            offer_asset: native_asset("test".to_string(), Uint128::new(1000000)),
            belief_price: None,
            max_spread: None,
            to: None,
        },
    )
    .unwrap_err();

    assert_eq!(err.to_string(), "Generic error: Must send reserve token 'test'".to_string());

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_1", &[Coin::new(1000000, "test")]),
        ExecuteMsg::Swap {
            offer_asset: native_asset("test".to_string(), Uint128::new(1000000)),
            belief_price: None,
            max_spread: None,
            to: None,
        },
    )
    .unwrap_err();

    assert_eq!(err.to_string(), "Expecting one of the supported tokens".to_string());

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_1", &[]),
        ExecuteMsg::Swap {
            offer_asset: native_asset("test".to_string(), Uint128::new(1000000)),
            belief_price: None,
            max_spread: None,
            to: None,
        },
    )
    .unwrap_err();

    assert_eq!(err.to_string(), "Generic error: No funds sent".to_string());

    // Bond when no delegation has been made
    // In this case, the full deposit simply goes to the first validator
    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_1", &[Coin::new(1000000, MOCK_UTOKEN)]),
        ExecuteMsg::Swap {
            offer_asset: native_asset(MOCK_UTOKEN.to_string(), Uint128::new(1000000)),
            belief_price: None,
            max_spread: None,
            to: None,
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
    assert_eq!(
        res.messages[index].msg,
        stake
            .dao_interface
            .deposit_msg(&stake.utoken, Uint128::new(1000000), MOCK_CONTRACT_ADDR.to_string())
            .unwrap()
    );

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
        ExecuteMsg::Swap {
            offer_asset: native_asset(MOCK_UTOKEN.to_string(), Uint128::new(12345)),
            belief_price: None,
            max_spread: None,
            to: Some("user_3".to_string()),
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
    assert_eq!(
        res.messages[index].msg,
        stake
            .dao_interface
            .deposit_msg(&stake.utoken, Uint128::new(12345), MOCK_CONTRACT_ADDR.to_string())
            .unwrap()
    );

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
    assert_eq!(
        res.messages[index].msg,
        stake
            .dao_interface
            .deposit_msg(&stake.utoken, Uint128::new(1000000), MOCK_CONTRACT_ADDR.to_string())
            .unwrap()
    );

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
            default_max_spread: None,
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
    assert_eq!(
        res.messages[0].msg,
        stake
            .dao_interface
            .deposit_msg(&stake.utoken, Uint128::new(12345), MOCK_CONTRACT_ADDR.to_string())
            .unwrap()
    );

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
    assert_eq!(
        vec![res.messages[0].msg.clone()],
        stake.dao_interface.claim_rewards_msgs(&mock_env(), &stake.utoken, vec![], vec![]).unwrap()
    );
    assert_eq!(res.messages[1], check_received_coin(0, 0));

    assert_eq!(
        res.messages[2],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_json_binary(&ExecuteMsg::Callback(CallbackMsg::Reinvest {
                skip_fee: false
            }))
            .unwrap(),
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
    assert_eq!(
        vec![res.messages[0].msg.clone()],
        stake.dao_interface.claim_rewards_msgs(&mock_env(), &stake.utoken, vec![], vec![]).unwrap()
    );
    assert_eq!(res.messages[1], check_received_coin(1000, 100));

    assert_eq!(
        res.messages[2],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_json_binary(&ExecuteMsg::Callback(CallbackMsg::Reinvest {
                skip_fee: false
            }))
            .unwrap(),
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
        ExecuteMsg::Callback(CallbackMsg::Reinvest {
            skip_fee: false,
        }),
    )
    .unwrap();

    assert_eq!(res.messages.len(), 4);

    let total = Uint128::from(234u128);
    let fee =
        Decimal::from_ratio(1u128, 100u128).checked_mul_uint(total).expect("expects fee result");
    let delegated = total.saturating_sub(fee);

    assert_eq!(
        res.messages[0].msg,
        stake
            .dao_interface
            .deposit_msg(&stake.utoken, delegated, MOCK_CONTRACT_ADDR.to_string())
            .unwrap()
    );
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
fn unbond() {
    let (mut deps, _) = setup_test();
    let state = State::default();

    // Only Stake token is accepted for unbonding requests
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("random_sender", &[Coin::new(100, "random_token")]),
        ExecuteMsg::Unbond {
            receiver: None,
        },
    )
    .unwrap_err();

    assert_eq!(err, ContractError::ExpectingStakeToken("random_token".into()));

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_1", &[Coin::new(1000000, MOCK_UTOKEN)]),
        ExecuteMsg::Bond {
            receiver: None,
            donate: None,
        },
    )
    .unwrap();

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_2", &[Coin::new(2000000, MOCK_UTOKEN)]),
        ExecuteMsg::Bond {
            receiver: None,
            donate: None,
        },
    )
    .unwrap();

    let stake = state.stake_token.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        stake,
        StakeToken {
            dao_interface: eris::hub::DaoInterface::Alliance {
                addr: Addr::unchecked("alliance")
            },
            denom: "factory/cosmos2contract/stake".to_string(),
            utoken: astroport::asset::AssetInfo::NativeToken {
                denom: "utoken".to_string()
            },
            total_supply: Uint128::new(3000000),
            total_utoken_bonded: Uint128::new(3000000),
        }
    );

    let res = execute(
        deps.as_mut(),
        mock_env_at_timestamp(269201), // est_unbond_start_time = 269200
        mock_info("user_2", &[Coin::new(69420, get_stake_full_denom())]),
        ExecuteMsg::Unbond {
            receiver: Some("user_3".to_string()),
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 3);
    assert_eq!(
        res.messages[0].msg,
        stake.dao_interface.unbond_msg(&stake.utoken, Uint128::new(69420)).unwrap()
    );
    assert_eq!(
        res.messages[1].msg,
        chain_test().create_burn_msg(get_stake_full_denom(), Uint128::new(69420))
    );
    assert_eq!(
        res.messages[2].msg,
        stake.utoken.with_balance(69420u128).transfer_msg(&Addr::unchecked("user_3")).unwrap()
    )
}

#[test]
fn swap_unbond() {
    let (mut deps, _) = setup_test();
    let state = State::default();

    // Only Stake token is accepted for unbonding requests
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("random_sender", &[Coin::new(100, "random_token")]),
        ExecuteMsg::Swap {
            offer_asset: native_asset("random_token".to_string(), Uint128::new(100)),
            belief_price: None,
            max_spread: None,
            to: None,
        },
    )
    .unwrap_err();

    assert_eq!(err, ContractError::ExpectingSupportedTokens {});

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_1", &[Coin::new(1000000, MOCK_UTOKEN)]),
        ExecuteMsg::Bond {
            receiver: None,
            donate: None,
        },
    )
    .unwrap();

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_2", &[Coin::new(2000000, MOCK_UTOKEN)]),
        ExecuteMsg::Bond {
            receiver: None,
            donate: None,
        },
    )
    .unwrap();

    let stake = state.stake_token.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        stake,
        StakeToken {
            dao_interface: eris::hub::DaoInterface::Alliance {
                addr: Addr::unchecked("alliance")
            },
            denom: "factory/cosmos2contract/stake".to_string(),
            utoken: astroport::asset::AssetInfo::NativeToken {
                denom: "utoken".to_string()
            },
            total_supply: Uint128::new(3000000),
            total_utoken_bonded: Uint128::new(3000000),
        }
    );

    let err = execute(
        deps.as_mut(),
        mock_env_at_timestamp(269201), // est_unbond_start_time = 269200
        mock_info("user_2", &[Coin::new(69420, get_stake_full_denom())]),
        ExecuteMsg::Swap {
            offer_asset: native_asset(get_stake_full_denom(), Uint128::new(69421)),
            belief_price: None,
            max_spread: None,
            to: Some("user_3".to_string()),
        },
    )
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        "Generic error: Native token balance mismatch between the argument and the transferred"
            .to_string()
    );

    let err = execute(
        deps.as_mut(),
        mock_env_at_timestamp(269201), // est_unbond_start_time = 269200
        mock_info("user_2", &[]),
        ExecuteMsg::Swap {
            offer_asset: native_asset(get_stake_full_denom(), Uint128::new(69421)),
            belief_price: None,
            max_spread: None,
            to: Some("user_3".to_string()),
        },
    )
    .unwrap_err();

    assert_eq!(err.to_string(), "Generic error: No funds sent".to_string());

    let err = execute(
        deps.as_mut(),
        mock_env_at_timestamp(269201), // est_unbond_start_time = 269200
        mock_info("user_2", &[Coin::new(69420, "random_token")]),
        ExecuteMsg::Swap {
            offer_asset: native_asset(get_stake_full_denom(), Uint128::new(69421)),
            belief_price: None,
            max_spread: None,
            to: Some("user_3".to_string()),
        },
    )
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        "Generic error: Must send reserve token 'factory/cosmos2contract/stake'".to_string()
    );

    let res = execute(
        deps.as_mut(),
        mock_env_at_timestamp(269201), // est_unbond_start_time = 269200
        mock_info("user_2", &[Coin::new(69420, get_stake_full_denom())]),
        ExecuteMsg::Swap {
            offer_asset: native_asset(get_stake_full_denom(), Uint128::new(69420)),
            belief_price: None,
            max_spread: None,
            to: Some("user_3".to_string()),
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 3);
    assert_eq!(
        res.messages[0].msg,
        stake.dao_interface.unbond_msg(&stake.utoken, Uint128::new(69420)).unwrap()
    );
    assert_eq!(
        res.messages[1].msg,
        chain_test().create_burn_msg(get_stake_full_denom(), Uint128::new(69420))
    );
    assert_eq!(
        res.messages[2].msg,
        stake.utoken.with_balance(69420u128).transfer_msg(&Addr::unchecked("user_3")).unwrap()
    )
}

#[test]
fn unbond_after_donate() {
    let (mut deps, _) = setup_test();
    let state = State::default();

    // Only Stake token is accepted for unbonding requests
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("random_sender", &[Coin::new(100, "random_token")]),
        ExecuteMsg::Unbond {
            receiver: None,
        },
    )
    .unwrap_err();

    assert_eq!(err, ContractError::ExpectingStakeToken("random_token".into()));

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_1", &[Coin::new(1000000, MOCK_UTOKEN)]),
        ExecuteMsg::Bond {
            receiver: None,
            donate: None,
        },
    )
    .unwrap();

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_2", &[Coin::new(2000000, MOCK_UTOKEN)]),
        ExecuteMsg::Bond {
            receiver: None,
            donate: None,
        },
    )
    .unwrap();

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("owner", &[]),
        ExecuteMsg::UpdateConfig {
            protocol_fee_contract: None,
            protocol_reward_fee: None,
            operator: None,
            stages_preset: None,
            withdrawals_preset: None,
            allow_donations: Some(true),
            default_max_spread: None,
        },
    )
    .unwrap();

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user_3", &[Coin::new(3000000, MOCK_UTOKEN)]),
        ExecuteMsg::Bond {
            receiver: None,
            donate: Some(true),
        },
    )
    .unwrap();

    let stake = state.stake_token.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        stake,
        StakeToken {
            dao_interface: eris::hub::DaoInterface::Alliance {
                addr: Addr::unchecked("alliance")
            },
            denom: "factory/cosmos2contract/stake".to_string(),
            utoken: astroport::asset::AssetInfo::NativeToken {
                denom: "utoken".to_string()
            },
            total_supply: Uint128::new(3000000),
            total_utoken_bonded: Uint128::new(6000000),
        }
    );

    let res = execute(
        deps.as_mut(),
        mock_env_at_timestamp(269201), // est_unbond_start_time = 269200
        mock_info("user_2", &[Coin::new(100000, get_stake_full_denom())]),
        ExecuteMsg::Unbond {
            receiver: Some("user_3".to_string()),
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 3);
    assert_eq!(
        res.messages[0].msg,
        stake.dao_interface.unbond_msg(&stake.utoken, Uint128::new(100000 * 2)).unwrap()
    );
    assert_eq!(
        res.messages[1].msg,
        chain_test().create_burn_msg(get_stake_full_denom(), Uint128::new(100000))
    );
    assert_eq!(
        res.messages[2].msg,
        stake.utoken.with_balance(100000u128 * 2).transfer_msg(&Addr::unchecked("user_3")).unwrap()
    );

    let stake = state.stake_token.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        stake,
        StakeToken {
            dao_interface: eris::hub::DaoInterface::Alliance {
                addr: Addr::unchecked("alliance")
            },
            denom: "factory/cosmos2contract/stake".to_string(),
            utoken: astroport::asset::AssetInfo::NativeToken {
                denom: "utoken".to_string()
            },
            total_supply: Uint128::new(3000000 - 100000),
            total_utoken_bonded: Uint128::new(6000000 - 100000 * 2),
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
            default_max_spread: None,
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
            default_max_spread: None,
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
            default_max_spread: None,
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
