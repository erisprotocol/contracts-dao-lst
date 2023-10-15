use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{coin, coins, to_binary, Addr, CosmosMsg, SubMsg, Uint128, WasmMsg};
use eris::hub::{CallbackMsg, ExecuteMsg};

use eris_chain_adapter::types::{MantaMsg, MantaSwap, MultiSwapRouterType};
use kujira::denom::Denom;

use crate::contract::execute;
use crate::error::ContractError;
use crate::state::State;
use crate::testing::helpers::{check_received_coin, get_stake_full_denom, setup_test, MOCK_UTOKEN};

#[test]
fn harvesting_with_balance() {
    let (mut deps, mut stake) = setup_test();
    stake.total_supply = Uint128::new(1000000);
    stake.total_utoken_bonded = Uint128::new(1000000);
    State::default().stake_token.save(deps.as_mut().storage, &stake).unwrap();

    deps.querier.set_bank_balances(&[
        coin(1000, MOCK_UTOKEN),
        coin(100, get_stake_full_denom()),
        coin(123, "usk"),
    ]);

    let mnta_msg = MantaMsg {
        swap: MantaSwap {
            stages: vec![],
            min_return: coins(100, "factory/umnta"),
        },
    };

    let denom: Denom = "usk".into();
    let router = (
        MultiSwapRouterType::Manta {
            addr: Addr::unchecked("mantaswap"),
            msg: mnta_msg.clone(),
        },
        vec![denom],
    );

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("worker", &[]),
        ExecuteMsg::Harvest {
            stages: None,
            withdrawals: None,
            cw20_assets: None,
            native_denoms: None,
            router: Some(router.clone()),
        },
    )
    .expect_err("Sender must be operator");
    assert_eq!(res, ContractError::UnauthorizedSenderNotOperator {});

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("operator", &[]),
        ExecuteMsg::Harvest {
            stages: None,
            withdrawals: None,
            cw20_assets: None,
            native_denoms: None,
            router: Some(router.clone()),
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 4);
    assert_eq!(
        res.messages[0].msg,
        stake.dao_interface.claim_rewards_msg(&mock_env(), vec![], vec![]).unwrap()
    );

    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::MultiSwapRouter {
                router: router.clone()
            }))
            .unwrap(),
            funds: vec![]
        }))
    );
    assert_eq!(res.messages[2], check_received_coin(1000, 100));

    assert_eq!(
        res.messages[3],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::Reinvest {})).unwrap(),
            funds: vec![]
        }))
    );

    // Check callback MultiSwapRouter
    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(MOCK_CONTRACT_ADDR, &[]),
        ExecuteMsg::Callback(CallbackMsg::MultiSwapRouter {
            router,
        }),
    )
    .unwrap();

    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "mantaswap".to_string(),
            msg: to_binary(&mnta_msg).unwrap(),
            funds: coins(123, "usk")
        }))
    );
}
