const CONTRACT_NAME: &str = "eris-ginkou-usdc-proxy";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

use crate::{
    error::{ContractError, ContractResult, CustomResult},
    ginkou::Ginkou,
    msg::{CallbackMsg, Config, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    state::CONFIG,
};
use astroport::asset::{native_asset_info, token_asset_info, AssetInfoExt, PairInfo};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Storage,
};
use cw2::set_contract_version;
use eris::{adapters::alliancehub::AllianceHub, helper::validate_received_funds, CustomMsgExt2};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    CONFIG.save(
        deps.storage,
        &Config {
            ginkou: Ginkou {
                contract: deps.api.addr_validate(&msg.ginkou)?,
                usdc_denom: msg.usdc_denom,
                musdc_addr: deps.api.addr_validate(&msg.musdc_addr)?,
            },
            hub: AllianceHub(deps.api.addr_validate(&msg.hub)?),
            stake: msg.stake,
        },
    )?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> ContractResult {
    match msg {
        ExecuteMsg::Bond {
            receiver,
            donate,
        } => {
            let config = CONFIG.load(deps.storage)?;
            bond(deps, env, info, config, receiver, donate)
        },

        ExecuteMsg::Unbond {
            receiver,
        } => {
            let config = CONFIG.load(deps.storage)?;
            unbond(deps, env, info, config, receiver)
        },

        ExecuteMsg::Swap {
            offer_asset,
            to,
            ..
        } => {
            offer_asset.assert_sent_native_token_balance(&info)?;

            let config = CONFIG.load(deps.storage)?;

            if offer_asset.info == native_asset_info(config.stake.clone()) {
                // unbond
                unbond(deps, env, info, config, to)
            } else if offer_asset.info == native_asset_info(config.ginkou.usdc_denom.clone()) {
                // bond
                bond(deps, env, info, config, to, None)
            } else {
                Err(ContractError::ExpectingSupportedToken {})
            }
        },
        ExecuteMsg::Callback(callback_msg) => callback(deps, env, info, callback_msg),
    }
}

fn bond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    config: Config,
    receiver: Option<String>,
    donate: Option<bool>,
) -> ContractResult {
    let usdc = native_asset_info(config.ginkou.usdc_denom.clone());
    let token_to_bond = validate_received_funds(&info.funds, &usdc)?;
    Ok(Response::new()
        .add_attribute("action", "erisproxy/bond")
        // 1. deposit
        .add_message(config.ginkou.deposit_usdc(token_to_bond.u128())?)
        // 2. bond
        .add_message(
            CallbackMsg::Bond {
                receiver: deps.api.addr_validate(&receiver.unwrap_or(info.sender.to_string()))?,
                donate,
            }
            .into_cosmos_msg(&env.contract.address)?,
        ))
}

fn unbond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    config: Config,
    receiver: Option<String>,
) -> ContractResult {
    let stake_token = native_asset_info(config.stake.clone());
    let token_to_unbond = validate_received_funds(&info.funds, &stake_token)?;
    let usdc = native_asset_info(config.ginkou.usdc_denom);

    Ok(Response::new()
        .add_attribute("action", "erisproxy/unbond")
        // 1. unbond
        .add_message(
            config.hub.unbond_msg(config.stake, token_to_unbond.u128(), None)?.to_normal()?,
        )
        // 2. withdraw
        .add_message(CallbackMsg::Unbond {}.into_cosmos_msg(&env.contract.address)?)
        // 3. send to receiver
        .add_message(send_to(deps, &env, receiver.unwrap_or(info.sender.to_string()), usdc)?))
}

fn send_to(
    deps: DepsMut,
    env: &Env,
    to: String,
    asset_info: astroport::asset::AssetInfo,
) -> CustomResult<CosmosMsg> {
    let receiver = deps.api.addr_validate(&to)?;

    Ok(CallbackMsg::SendTo {
        to: receiver,
        asset_info,
    }
    .into_cosmos_msg(&env.contract.address)?)
}

fn callback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    callback_msg: CallbackMsg,
) -> ContractResult {
    if env.contract.address != info.sender {
        return Err(ContractError::CallbackOnlyCalledByContract {});
    }

    match callback_msg {
        CallbackMsg::Bond {
            receiver,
            donate,
        } => {
            let config = CONFIG.load(deps.storage)?;
            let musdc = token_asset_info(config.ginkou.musdc_addr.clone());
            let balance = musdc.query_pool(&deps.querier, env.contract.address)?;

            Ok(Response::new().add_attribute("action", "erisproxy/callback-bond").add_message(
                config
                    .hub
                    .bond_msg(musdc.with_balance(balance), Some(receiver.to_string()), donate)?
                    .to_normal()?,
            ))
        },
        CallbackMsg::Unbond {} => {
            let config = CONFIG.load(deps.storage)?;
            let musdc = token_asset_info(config.ginkou.musdc_addr.clone());
            let balance = musdc.query_pool(&deps.querier, env.contract.address)?;

            Ok(Response::new()
                .add_attribute("action", "erisproxy/callback-unbond")
                .add_message(config.ginkou.withdraw_msg(balance)?))
        },
        CallbackMsg::SendTo {
            to: receiver,
            asset_info: token,
        } => {
            let amount = token.query_pool(&deps.querier, env.contract.address)?;
            let send = token.with_balance(amount).into_msg(receiver)?;

            Ok(Response::new()
                .add_attribute("action", "erisproxy/callback-sendto")
                .add_attribute("amount", amount)
                .add_message(send))
        },
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&get_config(deps.storage)?),
        QueryMsg::Pair {} => to_json_binary(&query_pair(deps, env)?),
    }
}

pub fn query_pair(deps: Deps, env: Env) -> StdResult<PairInfo> {
    let config = CONFIG.load(deps.storage)?;

    Ok(PairInfo {
        asset_infos: vec![
            native_asset_info(config.ginkou.usdc_denom),
            native_asset_info(config.stake),
        ],
        contract_addr: env.contract.address.clone(),
        liquidity_token: env.contract.address,
        pair_type: astroport::factory::PairType::Custom("virtual".to_string()),
    })
}

fn get_config(store: &dyn Storage) -> StdResult<Config> {
    CONFIG.load(store)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> ContractResult {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new()
        .add_attribute("new_contract_name", CONTRACT_NAME)
        .add_attribute("new_contract_version", CONTRACT_VERSION))
}
