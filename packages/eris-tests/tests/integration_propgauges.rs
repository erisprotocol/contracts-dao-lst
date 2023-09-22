use cosmwasm_std::{attr, StdResult};
use eris::governance_helper::WEEK;
use eris_tests::gov_helper::EscrowHelper;
use eris_tests::{mock_app, CustomAppExtension, EventChecker};

use eris::prop_gauges::{ConfigResponse, ExecuteMsg};

#[test]
fn integration_update_configs() -> StdResult<()> {
    let mut router = mock_app();
    let helper = EscrowHelper::init(&mut router);

    let config = helper.prop_query_config(&mut router).unwrap();
    assert_eq!(config.quorum_bps, 500u16);

    let result = helper
        .prop_execute_sender(
            &mut router,
            ExecuteMsg::UpdateConfig {
                quorum_bps: Some(100u16),
            },
            "user",
        )
        .unwrap_err();

    assert_eq!("Generic error: unauthorized", result.root_cause().to_string());

    helper
        .prop_execute(
            &mut router,
            ExecuteMsg::UpdateConfig {
                quorum_bps: Some(100u16),
            },
        )
        .unwrap();

    let config = helper.prop_query_config(&mut router).unwrap();
    assert_eq!(config.quorum_bps, 100u16);

    Ok(())
}

#[test]
fn vote() -> StdResult<()> {
    let mut router = mock_app();
    let helper = EscrowHelper::init(&mut router);
    let router_ref = &mut router;

    helper.cw4_stake(router_ref, 100_000000, "prop_creator".to_string()).unwrap();

    helper.mint_token(router_ref, "utoken", "user1", 100_000000);
    helper.hub_bond(router_ref, "user1", 100_000000, "utoken").unwrap();

    helper.ve_lock_lp(router_ref, "user1", 4000, 104 * WEEK).unwrap();
    helper.ve_lock_lp(router_ref, "user2", 4000, 104 * WEEK).unwrap();
    helper.ve_lock_lp(router_ref, "user3", 92000, 104 * WEEK).unwrap();

    // VOTE WITHOUT PROP
    let result =
        helper.prop_vote(router_ref, "user1", 10, cosmwasm_std::VoteOption::Yes).unwrap_err();
    assert_eq!(
        "Generic error: proposal with id 10 not initialized",
        result.root_cause().to_string()
    );

    // INIT THE PROPOSAL
    let result = helper
        .prop_init(router_ref, "user1", 10, router_ref.block_info().time.seconds() + 100)
        .unwrap_err();
    assert_eq!("Generic error: unauthorized", result.root_cause().to_string());

    let result = helper
        .prop_init(router_ref, "owner", 10, router_ref.block_info().time.seconds() + 3 * WEEK)
        .unwrap();

    result.assert_attribute("wasm", attr("action", "prop/init_prop")).unwrap();
    result.assert_attribute("wasm", attr("prop", "10")).unwrap();
    result.assert_attribute("wasm", attr("end", "3")).unwrap();

    helper
        .prop_init(router_ref, "owner", 11, router_ref.block_info().time.seconds() + 3 * WEEK)
        .unwrap();
    helper
        .prop_init(router_ref, "owner", 12, router_ref.block_info().time.seconds() + 3 * WEEK)
        .unwrap();

    // VOTE USER1
    let result = helper.prop_vote(router_ref, "user1", 10, cosmwasm_std::VoteOption::Yes).unwrap();
    result.assert_attribute("wasm", attr("action", "prop/vote")).unwrap();
    result.assert_attribute("wasm", attr("vp", "38946")).unwrap();

    // no matter the period, it is always the same VP (using different props, but same setup)
    router_ref.next_period(1);
    // VOTE USER1
    let result = helper.prop_vote(router_ref, "user1", 11, cosmwasm_std::VoteOption::Yes).unwrap();
    result.assert_attribute("wasm", attr("action", "prop/vote")).unwrap();
    result.assert_attribute("wasm", attr("vp", "38946")).unwrap();

    router_ref.next_period(1);
    // VOTE USER1
    let result = helper.prop_vote(router_ref, "user1", 12, cosmwasm_std::VoteOption::Yes).unwrap();
    result.assert_attribute("wasm", attr("action", "prop/vote")).unwrap();
    result.assert_attribute("wasm", attr("vp", "38946")).unwrap();

    // VOTE USER2
    let result =
        helper.prop_vote(router_ref, "user2", 10, cosmwasm_std::VoteOption::Yes).unwrap_err();
    assert_eq!("No vote operator set", result.root_cause().to_string());

    helper
        .hub_execute(
            router_ref,
            eris::hub::ExecuteMsg::UpdateConfig {
                protocol_fee_contract: None,
                protocol_reward_fee: None,
                allow_donations: None,
                vote_operator: Some(helper.base.prop_gauges.get_address_string()),
                default_max_spread: None,
                operator: None,
                stages_preset: None,
                withdrawals_preset: None,
                epoch_period: None,
                unbond_period: None,
            },
        )
        .unwrap();

    helper.cw3_create_20_props(router_ref, "prop_creator".to_string()).unwrap();

    let result = helper.prop_vote(router_ref, "user2", 10, cosmwasm_std::VoteOption::Yes).unwrap();
    result.assert_attribute("wasm", attr("action", "prop/vote")).unwrap();
    result.assert_attribute("wasm", attr("vp", "38946")).unwrap();
    Ok(())
}

#[test]
fn check_update_owner() -> StdResult<()> {
    let mut router = mock_app();
    let helper = EscrowHelper::init(&mut router);

    let new_owner = String::from("new_owner");

    // New owner
    let msg = ExecuteMsg::ProposeNewOwner {
        new_owner: new_owner.clone(),
        expires_in: 100, // seconds
    };

    // Unauthed check
    let err = helper.prop_execute_sender(&mut router, msg.clone(), "not_owner").unwrap_err();

    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    // Claim before proposal
    let err = helper
        .prop_execute_sender(&mut router, ExecuteMsg::ClaimOwnership {}, new_owner.clone())
        .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Ownership proposal not found");

    // Propose new owner
    helper.prop_execute_sender(&mut router, msg, "owner").unwrap();

    // Claim from invalid addr
    let err = helper
        .prop_execute_sender(&mut router, ExecuteMsg::ClaimOwnership {}, "invalid_addr")
        .unwrap_err();

    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    // Claim ownership
    helper
        .prop_execute_sender(&mut router, ExecuteMsg::ClaimOwnership {}, new_owner.clone())
        .unwrap();

    // Let's query the contract state
    let res: ConfigResponse = helper.prop_query_config(&mut router).unwrap();

    assert_eq!(res.owner, new_owner);
    Ok(())
}
