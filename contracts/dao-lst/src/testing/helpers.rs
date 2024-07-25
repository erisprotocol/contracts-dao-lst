use astroport::asset::{native_asset_info, Asset, AssetInfo, AssetInfoExt};
use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_json, to_json_binary, Addr, BlockInfo, ContractInfo, CosmosMsg, Decimal, Deps, Env,
    OwnedDeps, QuerierResult, StdResult, SubMsg, SystemError, SystemResult, Timestamp, Uint128,
    WasmMsg,
};
use eris_chain_adapter::types::{
    chain, CoinType, CustomMsgType, CustomQueryType, DenomType, MultiSwapRouterType, StageType,
    WithdrawType,
};
use serde::de::DeserializeOwned;

use eris::hub::{CallbackMsg, ExecuteMsg, InstantiateMsg, QueryMsg, StakeToken};

use crate::contract::{instantiate, query};
use crate::state::State;
use eris_chain_shared::chain_trait::ChainInterface;

use super::custom_querier::CustomQuerier;

pub const MOCK_UTOKEN: &str = "utoken";

pub(super) fn err_unsupported_query<T: std::fmt::Debug>(request: T) -> QuerierResult {
    SystemResult::Err(SystemError::InvalidRequest {
        error: format!("[mock] unsupported query: {:?}", request),
        request: Default::default(),
    })
}

pub(super) fn mock_dependencies() -> OwnedDeps<MockStorage, MockApi, CustomQuerier, CustomQueryType>
{
    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: CustomQuerier::default(),
        custom_query_type: std::marker::PhantomData,
    }
}

pub(super) fn mock_env_at_timestamp(timestamp: u64) -> Env {
    Env {
        block: BlockInfo {
            height: 12_345,
            time: Timestamp::from_seconds(timestamp),
            chain_id: "cosmos-testnet-14002".to_string(),
        },
        contract: ContractInfo {
            address: Addr::unchecked(MOCK_CONTRACT_ADDR),
        },
        transaction: None,
    }
}

pub(super) fn query_helper<T: DeserializeOwned>(deps: Deps<CustomQueryType>, msg: QueryMsg) -> T {
    from_json(query(deps, mock_env(), msg).unwrap()).unwrap()
}

pub(super) fn query_helper_env<T: DeserializeOwned>(
    deps: Deps<CustomQueryType>,
    msg: QueryMsg,
    timestamp: u64,
) -> T {
    from_json(query(deps, mock_env_at_timestamp(timestamp), msg).unwrap()).unwrap()
}

pub(super) fn get_stake_full_denom() -> String {
    // pub const STAKE_DENOM: &str = "factory/cosmos2contract/stake";
    chain(&mock_env()).get_token_denom(MOCK_CONTRACT_ADDR, "stake".into())
}

pub(super) fn set_total_stake_supply(
    state: &State,
    deps: &mut OwnedDeps<cosmwasm_std::MemoryStorage, MockApi, CustomQuerier, CustomQueryType>,
    total_supply: u128,
) {
    state
        .stake_token
        .update(deps.as_mut().storage, |mut stake| -> StdResult<StakeToken> {
            stake.total_supply = Uint128::new(total_supply);
            Ok(stake)
        })
        .unwrap();
}

pub fn check_received_coin(amount: u128, amount_stake: u128) -> SubMsg<CustomMsgType> {
    SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: MOCK_CONTRACT_ADDR.to_string(),
        msg: to_json_binary(&ExecuteMsg::Callback(CallbackMsg::CheckReceivedCoin {
            snapshot: mock_utoken_amount(amount),
            snapshot_stake: native_asset_info(get_stake_full_denom()).with_balance(amount_stake),
        }))
        .unwrap(),
        funds: vec![],
    }))
}

pub fn mock_utoken() -> AssetInfo {
    native_asset_info(MOCK_UTOKEN.to_string())
}

pub fn mock_utoken_amount(balance: impl Into<Uint128>) -> Asset {
    native_asset_info(MOCK_UTOKEN.to_string()).with_balance(balance)
}
//--------------------------------------------------------------------------------------------------
// Test setup
//--------------------------------------------------------------------------------------------------

pub(super) fn setup_test(
) -> (OwnedDeps<MockStorage, MockApi, CustomQuerier, CustomQueryType>, StakeToken) {
    let mut deps = mock_dependencies();

    let res = instantiate(
        deps.as_mut(),
        mock_env_at_timestamp(10000),
        mock_info("deployer", &[]),
        InstantiateMsg {
            owner: "owner".to_string(),
            denom: "stake".to_string(),
            utoken: mock_utoken(),
            epoch_period: 259200,   // 3 * 24 * 60 * 60 = 3 days
            unbond_period: 1814400, // 21 * 24 * 60 * 60 = 21 days
            protocol_fee_contract: "fee".to_string(),
            protocol_reward_fee: Decimal::from_ratio(1u128, 100u128),
            operator: "operator".to_string(),
            vote_operator: None,
            dao_interface: eris::hub::DaoInterface::Cw4 {
                addr: "cw4".to_string(),
                gov: "gov".to_string(),
                fund_distributor: "fund".to_string(),
            },
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 1);

    assert_eq!(
        res.messages[0].msg,
        chain_test().create_denom_msg(get_stake_full_denom(), "stake".to_string())
    );

    let state = State::default().stake_token.load(deps.as_ref().storage).unwrap();
    (deps, state)
}

pub fn chain_test(
) -> impl ChainInterface<CustomMsgType, DenomType, CoinType, WithdrawType, StageType, MultiSwapRouterType>
{
    chain(&mock_env())
}

pub trait WithoutGeneric {
    fn without_generic(&self) -> CosmosMsg;
}

impl WithoutGeneric for CosmosMsg<CustomMsgType> {
    fn without_generic(&self) -> CosmosMsg {
        match self.clone() {
            CosmosMsg::Bank(bank) => CosmosMsg::Bank(bank),
            CosmosMsg::Custom(_) => todo!(),
            CosmosMsg::Staking(bank) => CosmosMsg::Staking(bank),
            CosmosMsg::Distribution(bank) => CosmosMsg::Distribution(bank),
            CosmosMsg::Stargate {
                ..
            } => todo!(),
            CosmosMsg::Ibc(_) => todo!(),
            CosmosMsg::Wasm(bank) => CosmosMsg::Wasm(bank),
            CosmosMsg::Gov(bank) => CosmosMsg::Gov(bank),
            _ => todo!(),
        }
    }
}
