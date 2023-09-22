use std::str::FromStr;

use astroport::asset::native_asset_info;
use cosmwasm_schema::cw_serde;

use cosmwasm_std::{
    attr,
    testing::{MockApi, MockStorage},
    Addr, Decimal, Empty, GovMsg, IbcMsg, IbcQuery, StdResult, Uint128,
};
use cw20::{BalanceResponse, Cw20QueryMsg};

use cw_multi_test::{
    App, BankKeeper, ContractWrapper, DistributionKeeper, Executor, FailingModule, StakeKeeper,
    WasmKeeper,
};
use cw_utils::Duration;
use eris::arb_vault::LsdConfig;
// use eris::arb_vault::LsdConfig;
use eris_chain_adapter::types::{CustomMsgType, CustomQueryType};

use crate::{contracts::arb_contract, fund_distributor_contract, modules::types::UsedCustomModule};

pub const MULTIPLIER: u64 = 1_000_000;

#[cw_serde]
pub struct ContractInfo {
    pub address: Addr,
    pub code_id: u64,
}

#[cw_serde]
pub struct ContractInfoWrapper {
    contract: Option<ContractInfo>,
}

impl ContractInfoWrapper {
    pub fn get_address_string(&self) -> String {
        self.contract.clone().unwrap().address.to_string()
    }
    pub fn get_address(&self) -> Addr {
        self.contract.clone().unwrap().address
    }
}

impl From<Option<ContractInfo>> for ContractInfoWrapper {
    fn from(item: Option<ContractInfo>) -> Self {
        ContractInfoWrapper {
            contract: item,
        }
    }
}

#[cw_serde]
pub struct BaseErisTestPackage {
    pub owner: Addr,
    pub hub: ContractInfoWrapper,
    pub amp_token: ContractInfoWrapper,

    pub voting_escrow: ContractInfoWrapper,
    pub emp_gauges: ContractInfoWrapper,
    pub amp_gauges: ContractInfoWrapper,
    pub prop_gauges: ContractInfoWrapper,
    // pub amp_lp: ContractInfoWrapper,

    // pub stader: ContractInfoWrapper,
    // pub stader_reward: ContractInfoWrapper,
    // pub stader_token: ContractInfoWrapper,
    // pub steak_hub: ContractInfoWrapper,
    // pub steak_token: ContractInfoWrapper,
    pub cw3: ContractInfoWrapper,
    pub cw4: ContractInfoWrapper,
    pub fund: ContractInfoWrapper,

    pub arb_vault: ContractInfoWrapper,
    pub arb_fake_contract: ContractInfoWrapper,
}

#[cw_serde]
pub struct BaseErisTestInitMessage {
    pub owner: Addr,
}

pub type CustomApp = App<
    BankKeeper,
    MockApi,
    MockStorage,
    UsedCustomModule,
    WasmKeeper<CustomMsgType, CustomQueryType>,
    StakeKeeper,
    DistributionKeeper,
    FailingModule<IbcMsg, IbcQuery, Empty>,
    FailingModule<GovMsg, Empty, Empty>,
>;

impl BaseErisTestPackage {
    pub fn init_all(router: &mut CustomApp, msg: BaseErisTestInitMessage) -> Self {
        let mut base_pack = BaseErisTestPackage {
            owner: msg.owner.clone(),
            // token_id: None,
            // burnable_token_id: None,
            voting_escrow: None.into(),
            hub: None.into(),
            // amp_lp: None.into(),
            emp_gauges: None.into(),
            amp_gauges: None.into(),
            amp_token: None.into(),
            prop_gauges: None.into(),
            arb_vault: None.into(),
            arb_fake_contract: None.into(),
            // stader_token: None.into(),
            // stader: None.into(),
            // stader_reward: None.into(),
            cw3: None.into(),
            cw4: None.into(),
            fund: None.into(),
        };

        // base_pack.init_token(router, msg.owner.clone());
        base_pack.init_hub(router, msg.owner.clone());
        base_pack.init_voting_escrow(router, msg.owner.clone());

        base_pack.init_not_supported(router, msg.owner.clone());

        base_pack.init_arb_fake_contract(router, msg.owner.clone());

        base_pack
    }

    #[cfg(not(feature = "X-sei-X"))]
    fn init_not_supported(&mut self, router: &mut CustomApp, owner: Addr) {
        self.init_prop_gauges(router, owner.clone());
        // self.init_stader(router, msg.owner.clone());
        // self.init_steak_hub(router, owner.clone());
        self.init_arb_vault(router, owner.clone());
    }

    #[cfg(feature = "X-sei-X")]
    fn init_not_supported(&self) {}

    // fn init_token(&mut self, router: &mut CustomApp, owner: Addr) {
    //     let contract = Box::new(ContractWrapper::new_with_empty(
    //         eris_staking_token::execute,
    //         eris_staking_token::instantiate,
    //         eris_staking_token::query,
    //     ));

    //     let token_code_id = router.store_code(contract);
    //     self.token_id = Some(token_code_id);

    //     let contract = Box::new(ContractWrapper::new_with_empty(
    //         cw20_base::contract::execute,
    //         cw20_base::contract::instantiate,
    //         cw20_base::contract::query,
    //     ));

    //     let token_code_id = router.store_code(contract);
    //     self.burnable_token_id = Some(token_code_id);

    //     let init_msg = cw20_base::msg::InstantiateMsg {
    //         name: "ampLP".to_string(),
    //         symbol: "stake".to_string(),
    //         decimals: 6,
    //         initial_balances: vec![],
    //         mint: Some(MinterResponse {
    //             minter: owner.to_string(),
    //             cap: None,
    //         }),
    //         marketing: None,
    //     };

    //     let instance = router
    //         .instantiate_contract(self.token_id.unwrap(), owner, &init_msg, &[], "Hub", None)
    //         .unwrap();

    //     self.amp_lp = Some(ContractInfo {
    //         address: instance,
    //         code_id: self.token_id.unwrap(),
    //     })
    //     .into()
    // }

    fn init_hub(&mut self, router: &mut CustomApp, owner: Addr) {
        let cw4_contract = Box::new(ContractWrapper::new_with_empty(
            manta_stake::contract::execute,
            manta_stake::contract::instantiate,
            manta_stake::contract::query,
        ));
        let cw4_code_id = router.store_code(cw4_contract);
        let cw4_instance = router
            .instantiate_contract(
                cw4_code_id,
                owner.clone(),
                &manta_stake::msg::InstantiateMsg {
                    admin: Some("admin".to_string()),
                    min_bond: Uint128::new(1000000u128),
                    unbonding_period: Duration::Time(21 * 24 * 60 * 60).into(),
                    denom: manta_cw20::Denom::Native("utoken".to_string()),
                    tokens_per_weight: Uint128::new(1000000u128),
                },
                &[],
                "Cw4",
                None,
            )
            .unwrap();
        self.cw4 = Some(ContractInfo {
            address: cw4_instance.clone(),
            code_id: cw4_code_id,
        })
        .into();

        let cw3_contract = Box::new(ContractWrapper::new_with_empty(
            manta_cw3::contract::execute,
            manta_cw3::contract::instantiate,
            manta_cw3::contract::query,
        ));
        let cw3_code_id = router.store_code(cw3_contract);
        let cw3_instance = router
            .instantiate_contract(
                cw3_code_id,
                owner.clone(),
                &manta_cw3::msg::InstantiateMsg {
                    group_addr: cw4_instance.to_string(),
                    threshold: cw_utils::Threshold::ThresholdQuorum {
                        threshold: Decimal::from_str("0.5").unwrap(),
                        quorum: Decimal::from_str("0.3").unwrap(),
                    },
                    max_voting_period: Duration::Time(259200),
                    executor: None,
                    proposal_deposit: None,
                },
                &[],
                "Cw3",
                None,
            )
            .unwrap();
        self.cw3 = Some(ContractInfo {
            address: cw3_instance.clone(),
            code_id: cw3_code_id,
        })
        .into();

        let fund_contract = Box::new(ContractWrapper::new_with_empty(
            fund_distributor_contract::execute,
            fund_distributor_contract::instantiate,
            fund_distributor_contract::query,
        ));
        let fund_code_id = router.store_code(fund_contract);
        let fund_instance = router
            .instantiate_contract(
                fund_code_id,
                owner.clone(),
                &fund_distributor_contract::InstantiateMsg {},
                &[],
                "Fund distributor",
                None,
            )
            .unwrap();
        self.fund = Some(ContractInfo {
            address: fund_instance.clone(),
            code_id: fund_code_id,
        })
        .into();

        let hub_contract = Box::new(ContractWrapper::new(
            eris_staking_hub::contract::execute,
            eris_staking_hub::contract::instantiate,
            eris_staking_hub::contract::query,
        ));

        let code_id = router.store_code(hub_contract);

        let init_msg = eris::hub::InstantiateMsg {
            denom: "ampSTAKE".into(),
            operator: "operator".to_string(),
            utoken: native_asset_info("utoken".to_string()),
            owner: owner.to_string(),
            epoch_period: 1 * 24 * 60 * 60,   // 1 day
            unbond_period: 21 * 24 * 60 * 60, // 21 days
            protocol_fee_contract: "fee".to_string(),
            protocol_reward_fee: Decimal::from_ratio(1u128, 100u128),
            vote_operator: None,
            dao_interface: eris::hub::DaoInterface::CW4 {
                addr: cw4_instance.to_string(),
                gov: cw3_instance.to_string(),
                fund_distributor: fund_instance.to_string(),
            },
        };

        let instance =
            router.instantiate_contract(code_id, owner, &init_msg, &[], "Hub", None).unwrap();

        let config: eris::hub::ConfigResponse = router
            .wrap()
            .query_wasm_smart(instance.to_string(), &eris::hub::QueryMsg::Config {})
            .unwrap();

        self.amp_token = Some(ContractInfo {
            address: Addr::unchecked(config.stake_token),
            code_id: 0,
        })
        .into();

        self.hub = Some(ContractInfo {
            address: instance,
            code_id,
        })
        .into()
    }

    fn init_voting_escrow(&mut self, router: &mut CustomApp, owner: Addr) {
        let voting_contract = Box::new(ContractWrapper::new_with_empty(
            eris_gov_voting_escrow::contract::execute,
            eris_gov_voting_escrow::contract::instantiate,
            eris_gov_voting_escrow::contract::query,
        ));

        let voting_code_id = router.store_code(voting_contract);

        let msg = eris::voting_escrow::InstantiateMsg {
            guardian_addr: Some("guardian".to_string()),
            marketing: None,
            owner: owner.to_string(),
            deposit_denom: self.amp_token.get_address_string(),
            logo_urls_whitelist: vec![],
        };

        let voting_instance = router
            .instantiate_contract(voting_code_id, owner, &msg, &[], String::from("vxASTRO"), None)
            .unwrap();

        self.voting_escrow = Some(ContractInfo {
            address: voting_instance,
            code_id: voting_code_id,
        })
        .into()
    }

    #[cfg(not(feature = "X-sei-X"))]
    fn init_prop_gauges(&mut self, router: &mut CustomApp, owner: Addr) {
        let contract = Box::new(ContractWrapper::new(
            eris_gov_prop_gauges::contract::execute,
            eris_gov_prop_gauges::contract::instantiate,
            eris_gov_prop_gauges::contract::query,
        ));

        let code_id = router.store_code(contract);

        let msg = eris::prop_gauges::InstantiateMsg {
            owner: owner.to_string(),
            hub_addr: self.hub.get_address_string(),
            escrow_addr: self.voting_escrow.get_address_string(),
            quorum_bps: 500,
        };

        let instance = router
            .instantiate_contract(code_id, owner, &msg, &[], String::from("prop-gauges"), None)
            .unwrap();

        self.prop_gauges = Some(ContractInfo {
            address: instance,
            code_id,
        })
        .into()
    }

    #[cfg(not(feature = "X-sei-X"))]
    fn init_arb_vault(&mut self, router: &mut CustomApp, owner: Addr) {
        let contract = Box::new(
            ContractWrapper::new(
                eris_arb_vault::contract::execute,
                eris_arb_vault::contract::instantiate,
                eris_arb_vault::contract::query,
            ), // .with_reply(eris_arb_vault::contract::reply),
        );

        let code_id = router.store_code(contract);
        let hub_addr = self.hub.get_address();

        let msg = eris::arb_vault::InstantiateMsg {
            owner: owner.to_string(),

            denom: "arbLUNA".into(),
            fee_config: eris::arb_vault::FeeConfig {
                protocol_fee_contract: "fee".to_string(),
                protocol_performance_fee: Decimal::from_str("0.1").unwrap(),
                protocol_withdraw_fee: Decimal::from_str("0.01").unwrap(),
                immediate_withdraw_fee: Decimal::from_str("0.03").unwrap(),
            },
            unbond_time_s: 24 * 24 * 60 * 60,
            utilization_method: eris::arb_vault::UtilizationMethod::Steps(vec![
                (Decimal::from_ratio(10u128, 1000u128), Decimal::from_ratio(50u128, 100u128)),
                (Decimal::from_ratio(15u128, 1000u128), Decimal::from_ratio(70u128, 100u128)),
                (Decimal::from_ratio(20u128, 1000u128), Decimal::from_ratio(90u128, 100u128)),
                (Decimal::from_ratio(25u128, 1000u128), Decimal::from_ratio(100u128, 100u128)),
            ]),
            utoken: "uluna".to_string(),
            whitelist: vec!["executor".to_string()],
            lsds: vec![LsdConfig {
                name: "eris".into(),
                lsd_type: eris::arb_vault::LsdType::Eris {
                    addr: hub_addr.to_string(),
                    denom: self.amp_token.get_address_string(),
                },
                disabled: false,
            }],
        };

        let instance = router
            .instantiate_contract(code_id, owner, &msg, &[], String::from("arb-vault"), None)
            .unwrap();

        self.arb_vault = Some(ContractInfo {
            address: instance,
            code_id,
        })
        .into()
    }

    // fn init_stader(&mut self, router: &mut CustomApp, owner: Addr) {
    //     self.init_stader_reward(router, owner.clone());

    //     let contract = Box::new(ContractWrapper::new_with_empty(
    //         stader::contract::execute,
    //         stader::contract::instantiate,
    //         stader::contract::query,
    //     ));

    //     let code_id = router.store_code(contract);

    //     let msg = stader::msg::InstantiateMsg {
    //         min_deposit: Uint128::new(1000),
    //         max_deposit: Uint128::new(1000000000000),
    //         airdrop_withdrawal_contract: "terra1gq5fgg5wtlcnhtf0w2swun8r7zdvydyeazda8u".to_string(),
    //         airdrops_registry_contract:
    //             "terra1fvw0rt94gl5eyeq36qdhj5x7lunv3xpuqcjxa0llhdssvqtcmrnqlzxdyr".to_string(),
    //         protocol_deposit_fee: Decimal::percent(0),
    //         protocol_fee_contract: "stader_fee".to_string(),
    //         protocol_reward_fee: Decimal::percent(0),
    //         protocol_withdraw_fee: Decimal::zero(),
    //         reinvest_cooldown: 3600,
    //         reward_contract: self.stader_reward.get_address_string(),
    //         unbonding_period: 1815300,
    //         undelegation_cooldown: 259000,
    //     };

    //     let instance = router
    //         .instantiate_contract(
    //             code_id,
    //             owner.clone(),
    //             &msg,
    //             &[],
    //             String::from("stader-hub"),
    //             None,
    //         )
    //         .unwrap();

    //     self.stader = Some(ContractInfo {
    //         address: instance,
    //         code_id,
    //     })
    //     .into();

    //     // init token

    //     let init_msg = cw20_base::msg::InstantiateMsg {
    //         name: "LunaX".to_string(),
    //         symbol: "LUNAX".to_string(),
    //         decimals: 6,
    //         initial_balances: vec![],
    //         mint: Some(MinterResponse {
    //             minter: self.stader.get_address_string(),
    //             cap: None,
    //         }),
    //         marketing: None,
    //     };
    //     let stader_token_instance = router
    //         .instantiate_contract(
    //             self.token_id.unwrap(),
    //             owner.clone(),
    //             &init_msg,
    //             &[],
    //             String::from("stader-token"),
    //             None,
    //         )
    //         .unwrap();

    //     self.stader_token = Some(ContractInfo {
    //         address: stader_token_instance.clone(),
    //         code_id: self.token_id.unwrap(),
    //     })
    //     .into();

    //     // update config reward
    //     router
    //         .execute_contract(
    //             owner,
    //             self.stader_reward.get_address(),
    //             &stader_reward::msg::ExecuteMsg::UpdateConfig {
    //                 staking_contract: Some(self.stader.get_address_string()),
    //             },
    //             &[],
    //         )
    //         .unwrap();

    //     // update config hub
    //     router
    //         .execute_contract(
    //             self.owner.clone(),
    //             self.stader.get_address(),
    //             &StaderExecuteMsg::UpdateConfig {
    //                 config_request: StaderConfigUpdateRequest {
    //                     min_deposit: None,
    //                     max_deposit: None,
    //                     cw20_token_contract: Some(stader_token_instance.to_string()),
    //                     protocol_reward_fee: None,
    //                     protocol_withdraw_fee: None,
    //                     protocol_deposit_fee: None,
    //                     airdrop_registry_contract: None,
    //                     unbonding_period: None,
    //                     undelegation_cooldown: None,
    //                     reinvest_cooldown: None,
    //                 },
    //             },
    //             &[],
    //         )
    //         .unwrap();

    //     // add validators hub
    //     router
    //         .execute_contract(
    //             self.owner.clone(),
    //             self.stader.get_address(),
    //             &stader::msg::ExecuteMsg::AddValidator {
    //                 val_addr: Addr::unchecked("val1"),
    //             },
    //             &[],
    //         )
    //         .unwrap();
    // }

    // fn init_stader_reward(&mut self, router: &mut CustomApp, owner: Addr) {
    //     let contract = Box::new(ContractWrapper::new_with_empty(
    //         stader_reward::contract::execute,
    //         stader_reward::contract::instantiate,
    //         stader_reward::contract::query,
    //     ));

    //     let code_id = router.store_code(contract);

    //     let msg = stader_reward::msg::InstantiateMsg {
    //         staking_contract: "any".into(),
    //     };

    //     let instance = router
    //         .instantiate_contract(code_id, owner, &msg, &[], String::from("stader-hub"), None)
    //         .unwrap();

    //     self.stader_reward = Some(ContractInfo {
    //         address: instance,
    //         code_id,
    //     })
    //     .into();
    // }

    fn init_arb_fake_contract(&mut self, router: &mut CustomApp, owner: Addr) {
        let contract = Box::new(ContractWrapper::new_with_empty(
            arb_contract::execute,
            arb_contract::instantiate,
            arb_contract::query,
        ));
        let code_id = router.store_code(contract);

        let instance = router
            .instantiate_contract(
                code_id,
                owner,
                &arb_contract::InstantiateMsg {},
                &[],
                String::from("arb-fake-contract"),
                None,
            )
            .unwrap();

        self.arb_fake_contract = Some(ContractInfo {
            address: instance,
            code_id,
        })
        .into()
    }
}

pub fn mint(router: &mut CustomApp, owner: Addr, token_instance: Addr, to: &Addr, amount: u128) {
    let amount = amount * MULTIPLIER as u128;
    let msg = cw20::Cw20ExecuteMsg::Mint {
        recipient: to.to_string(),
        amount: Uint128::from(amount),
    };

    let res = router.execute_contract(owner, token_instance, &msg, &[]).unwrap();
    assert_eq!(res.events[1].attributes[1], attr("action", "mint"));
    assert_eq!(res.events[1].attributes[2], attr("to", String::from(to)));
    assert_eq!(res.events[1].attributes[3], attr("amount", Uint128::from(amount)));
}

pub fn check_balance(app: &mut CustomApp, token_addr: &Addr, contract_addr: &Addr, expected: u128) {
    let msg = Cw20QueryMsg::Balance {
        address: contract_addr.to_string(),
    };
    let res: StdResult<BalanceResponse> = app.wrap().query_wasm_smart(token_addr, &msg);
    assert_eq!(res.unwrap().balance, Uint128::from(expected));
}

pub fn increase_allowance(
    router: &mut CustomApp,
    owner: Addr,
    spender: Addr,
    token: Addr,
    amount: Uint128,
) {
    let msg = cw20::Cw20ExecuteMsg::IncreaseAllowance {
        spender: spender.to_string(),
        amount,
        expires: None,
    };

    let res = router.execute_contract(owner.clone(), token, &msg, &[]).unwrap();

    assert_eq!(res.events[1].attributes[1], attr("action", "increase_allowance"));
    assert_eq!(res.events[1].attributes[2], attr("owner", owner.to_string()));
    assert_eq!(res.events[1].attributes[3], attr("spender", spender.to_string()));
    assert_eq!(res.events[1].attributes[4], attr("amount", amount));
}
