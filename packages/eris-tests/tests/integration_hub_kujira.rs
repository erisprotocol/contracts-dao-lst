#[cfg(feature = "X-kujira-X")]
mod kujira {

    use std::str::FromStr;

    use cosmwasm_std::{coin, Addr, Decimal, StdResult, Uint128};
    use eris_chain_adapter::types::{MantaMsg, MantaSwap, MultiSwapRouterType};
    use eris_tests::gov_helper::EscrowHelper;
    use eris_tests::mock_app;

    type RouterRefType = cw_multi_test::App<
        cw_multi_test::BankKeeper,
        cosmwasm_std::testing::MockApi,
        cosmwasm_std::MemoryStorage,
        eris_tests::modules::kujira::KujiraModule,
        cw_multi_test::WasmKeeper<kujira::msg::KujiraMsg, cosmwasm_std::Empty>,
    >;

    #[test]
    fn integration_bond_donate() -> StdResult<()> {
        let mut router = mock_app();
        let helper = &EscrowHelper::init(&mut router);
        let router_ref: &mut RouterRefType = &mut router;

        helper.mint_utoken(router_ref, "user", 100_000000);
        helper.mint_utoken(router_ref, "user2", 100_000000);

        check_bond(helper, router_ref);
        check_donate(helper, router_ref);

        check_harvest_not_operator(helper, router_ref);
        check_harvest_no_tokens(helper, router_ref);

        check_harvest_utoken(helper, router_ref);
        check_harvest_amptoken(helper, router_ref);
        check_harvest_test_with_swap(helper, router_ref);
        check_harvest_test_with_swap_not_allowed(helper, router_ref);

        Ok(())
    }

    fn check_harvest_test_with_swap_not_allowed(
        helper: &EscrowHelper,
        router_ref: &mut RouterRefType,
    ) {
        helper.mint_token(router_ref, "test", helper.base.fund.get_address_string(), 1_000000);
        let utoken = helper.base.utoken_denom();
        let amptoken = helper.base.amp_token.get_address_string();

        helper.mint_token(
            router_ref,
            utoken.clone(),
            helper.base.swap.get_address_string(),
            1_000000,
        );

        let res = helper
            .hub_execute_sender(
                router_ref,
                eris::hub::ExecuteMsg::Harvest {
                    native_denoms: None,
                    cw20_assets: None,
                    withdrawals: None,
                    stages: None,
                    router: Some((
                        MultiSwapRouterType::Manta {
                            addr: helper.base.swap.get_address(),
                            msg: MantaMsg {
                                swap: MantaSwap {
                                    stages: vec![vec![("test".to_string(), utoken.clone())]],
                                    min_return: vec![coin(1_000000, utoken.clone())],
                                },
                            },
                        },
                        vec!["test".into(), utoken.clone().into()],
                    )),
                },
                Addr::unchecked("operator"),
            )
            .unwrap_err();

        assert_eq!(res.root_cause().to_string(), format!("Swap from {} is not allowed", utoken));

        let res = helper
            .hub_execute_sender(
                router_ref,
                eris::hub::ExecuteMsg::Harvest {
                    native_denoms: None,
                    cw20_assets: None,
                    withdrawals: None,
                    stages: None,
                    router: Some((
                        MultiSwapRouterType::Manta {
                            addr: helper.base.swap.get_address(),
                            msg: MantaMsg {
                                swap: MantaSwap {
                                    stages: vec![vec![("test".to_string(), utoken.clone())]],
                                    min_return: vec![coin(1_000000, utoken)],
                                },
                            },
                        },
                        vec!["test".into(), amptoken.clone().into()],
                    )),
                },
                Addr::unchecked("operator"),
            )
            .unwrap_err();

        assert_eq!(res.root_cause().to_string(), format!("Swap from {} is not allowed", amptoken));
    }

    fn check_harvest_test_with_swap(helper: &EscrowHelper, router_ref: &mut RouterRefType) {
        helper.mint_token(router_ref, "test", helper.base.fund.get_address_string(), 1_000000);
        let utoken = helper.base.utoken_denom();

        helper.mint_token(
            router_ref,
            utoken.clone(),
            helper.base.swap.get_address_string(),
            1_000000,
        );

        helper
            .hub_execute_sender(
                router_ref,
                eris::hub::ExecuteMsg::Harvest {
                    native_denoms: None,
                    cw20_assets: None,
                    withdrawals: None,
                    stages: None,
                    router: Some((
                        MultiSwapRouterType::Manta {
                            addr: helper.base.swap.get_address(),
                            msg: MantaMsg {
                                swap: MantaSwap {
                                    stages: vec![vec![("test".to_string(), utoken.clone())]],
                                    min_return: vec![coin(1_000000, utoken)],
                                },
                            },
                        },
                        vec!["test".into()],
                    )),
                },
                Addr::unchecked("operator"),
            )
            .unwrap();

        let state = helper.hub_query_state(router_ref).unwrap();
        assert_eq!(
            state,
            eris::hub::StateResponse {
                total_ustake: Uint128::new(2010000),
                total_utoken: Uint128::new(7980000),
                exchange_rate: Decimal::from_str("3.970149253731343283").unwrap(),
                unlocked_coins: vec![],
                unbonding: Uint128::zero(),
                available: Uint128::zero(),
                tvl_utoken: Uint128::new(7980000)
            }
        );
    }

    fn check_harvest_amptoken(helper: &EscrowHelper, router_ref: &mut RouterRefType) {
        helper.mint_token(
            router_ref,
            helper.base.amp_token.get_address_string(),
            helper.base.fund.get_address_string(),
            1_000000,
        );

        helper
            .hub_execute_sender(
                router_ref,
                eris::hub::ExecuteMsg::Harvest {
                    native_denoms: None,
                    cw20_assets: None,
                    withdrawals: None,
                    stages: None,
                    router: None,
                },
                Addr::unchecked("operator"),
            )
            .unwrap();

        let state = helper.hub_query_state(router_ref).unwrap();
        assert_eq!(
            state,
            eris::hub::StateResponse {
                total_ustake: Uint128::new(2010000),
                total_utoken: Uint128::new(6990000),
                exchange_rate: Decimal::from_str("3.477611940298507462").unwrap(),
                unlocked_coins: vec![],
                unbonding: Uint128::zero(),
                available: Uint128::zero(),
                tvl_utoken: Uint128::new(6990000)
            }
        );
    }

    fn check_harvest_utoken(helper: &EscrowHelper, router_ref: &mut RouterRefType) {
        helper.mint_token(
            router_ref,
            helper.base.utoken_denom(),
            helper.base.fund.get_address_string(),
            1_000000,
        );

        helper
            .hub_execute_sender(
                router_ref,
                eris::hub::ExecuteMsg::Harvest {
                    native_denoms: None,
                    cw20_assets: None,
                    withdrawals: None,
                    stages: None,
                    router: None,
                },
                Addr::unchecked("operator"),
            )
            .unwrap();

        let state = helper.hub_query_state(router_ref).unwrap();
        assert_eq!(
            state,
            eris::hub::StateResponse {
                total_ustake: Uint128::new(3000000),
                total_utoken: Uint128::new(6990000),
                exchange_rate: Decimal::from_str("2.33").unwrap(),
                unlocked_coins: vec![],
                unbonding: Uint128::zero(),
                available: Uint128::zero(),
                tvl_utoken: Uint128::new(6990000)
            }
        );
    }

    fn check_harvest_no_tokens(helper: &EscrowHelper, router_ref: &mut RouterRefType) {
        helper.mint_token(router_ref, "test", helper.base.fund.get_address_string(), 10_000000);

        let result = helper
            .hub_execute_sender(
                router_ref,
                eris::hub::ExecuteMsg::Harvest {
                    native_denoms: None,
                    cw20_assets: None,
                    withdrawals: None,
                    stages: None,
                    router: None,
                },
                Addr::unchecked("operator"),
            )
            .unwrap_err();

        assert_eq!(
            result.root_cause().to_string(),
            format!(
                "No {0}, {1} available to be bonded",
                helper.base.utoken_denom(),
                helper.base.amp_token.get_address_string()
            )
        );
    }

    fn check_harvest_not_operator(helper: &EscrowHelper, router_ref: &mut RouterRefType) {
        let result = helper
            .hub_execute(
                router_ref,
                eris::hub::ExecuteMsg::Harvest {
                    native_denoms: None,
                    cw20_assets: None,
                    withdrawals: None,
                    stages: None,
                    router: Some((
                        MultiSwapRouterType::Manta {
                            addr: Addr::unchecked("mantaswap"),
                            msg: MantaMsg {
                                swap: MantaSwap {
                                    stages: vec![],
                                    min_return: vec![],
                                },
                            },
                        },
                        vec!["test".into()],
                    )),
                },
            )
            .unwrap_err();

        assert_eq!(
            result.root_cause().to_string(),
            "Unauthorized: sender is not operator".to_string()
        );
    }

    fn check_bond(helper: &EscrowHelper, router_ref: &mut RouterRefType) {
        helper.hub_bond(router_ref, "user", 1000000, helper.base.utoken_denom()).unwrap();
        helper.hub_bond(router_ref, "user2", 2000000, helper.base.utoken_denom()).unwrap();

        let state = helper.hub_query_state(router_ref).unwrap();
        assert_eq!(
            state,
            eris::hub::StateResponse {
                total_ustake: Uint128::new(3000000),
                total_utoken: Uint128::new(3000000),
                exchange_rate: Decimal::from_str("1").unwrap(),
                unlocked_coins: vec![],
                unbonding: Uint128::zero(),
                available: Uint128::zero(),
                tvl_utoken: Uint128::new(3000000)
            }
        );
    }

    fn check_donate(helper: &EscrowHelper, router_ref: &mut RouterRefType) {
        // check donate
        helper.hub_allow_donate(router_ref).unwrap();
        helper.hub_donate(router_ref, "user2", 3000000, helper.base.utoken_denom()).unwrap();

        let state = helper.hub_query_state(router_ref).unwrap();
        assert_eq!(
            state,
            eris::hub::StateResponse {
                total_ustake: Uint128::new(3000000),
                total_utoken: Uint128::new(6000000),
                exchange_rate: Decimal::from_str("2").unwrap(),
                unlocked_coins: vec![],
                unbonding: Uint128::zero(),
                available: Uint128::zero(),
                tvl_utoken: Uint128::new(6000000)
            }
        );
    }
}
