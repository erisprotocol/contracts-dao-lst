const CONTRACT_NAME: &str = "eris-ginkou-usdc-proxy";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

use crate::{
    error::{ContractError, ContractResult, CustomResult},
    ginkou::Ginkou,
    msg::{CallbackMsg, Config, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, ReceiveMsg},
    state::CONFIG,
};
use astroport::asset::{native_asset_info, token_asset_info, AssetInfoExt, PairInfo};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Storage,
};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;

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
        },
    )?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> ContractResult {
    match msg {
        ExecuteMsg::Receive(cw20_msg) => receive(deps, env, info, cw20_msg),
        ExecuteMsg::Swap {
            offer_asset,
            to,
            ..
        } => {
            offer_asset.assert_sent_native_token_balance(&info)?;
            let config = CONFIG.load(deps.storage)?;

            if offer_asset.info != native_asset_info(config.ginkou.usdc_denom.clone()) {
                return Err(ContractError::ExpectingUsdcToken(offer_asset.info.to_string()));
            }

            Ok(Response::new()
                .add_attribute("action", "virtual-swap")
                .add_message(config.ginkou.deposit_usdc(offer_asset.amount.u128())?)
                .add_message(send_to(
                    deps,
                    &env,
                    to.unwrap_or(info.sender.to_string()),
                    token_asset_info(config.ginkou.musdc_addr),
                )?))
        },
        ExecuteMsg::Callback(callback_msg) => callback(deps, env, info, callback_msg),
    }
}

fn receive(deps: DepsMut, env: Env, info: MessageInfo, cw20_msg: Cw20ReceiveMsg) -> ContractResult {
    match from_binary(&cw20_msg.msg)? {
        ReceiveMsg::Swap {
            to,
            ..
        } => {
            let config = CONFIG.load(deps.storage)?;

            if info.sender != config.ginkou.musdc_addr {
                return Err(ContractError::ExpectingMusdcToken(info.sender.into()));
            }

            Ok(Response::new()
                .add_attribute("action", "virtual-swap_receive")
                .add_message(config.ginkou.withdraw_msg(cw20_msg.amount)?)
                .add_message(send_to(
                    deps,
                    &env,
                    to.unwrap_or(info.sender.to_string()),
                    native_asset_info(config.ginkou.usdc_denom),
                )?))
        },
    }
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
        CallbackMsg::SendTo {
            to: receiver,
            asset_info: token,
        } => {
            let amount = token.query_pool(&deps.querier, env.contract.address)?;
            let send = token.with_balance(amount).into_msg(receiver)?;

            Ok(Response::new().add_attribute("action", "callback-send_to").add_message(send))
        },
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&get_config(deps.storage)?),
        QueryMsg::Pair {} => to_binary(&query_pair(deps, env)?),
    }
}

pub fn query_pair(deps: Deps, env: Env) -> StdResult<PairInfo> {
    let config = CONFIG.load(deps.storage)?;

    Ok(PairInfo {
        asset_infos: vec![
            native_asset_info(config.ginkou.usdc_denom),
            token_asset_info(config.ginkou.musdc_addr),
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
