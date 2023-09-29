use astroport::asset::{native_asset_info, AssetInfoExt};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    entry_point, Addr, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
};
use eris::{adapters::asset::AssetEx, CustomMsgExt2};

pub type ContractResult = Result<Response, StdError>;

#[cw_serde]
pub enum ExecuteMsg {
    Swap {
        stages: Vec<Vec<(Addr, String)>>,
        recipient: Option<Addr>,
        min_return: Option<Vec<Coin>>,
    },
}

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum QueryMsg {}

#[entry_point]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> ContractResult {
    Ok(Response::new())
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> ContractResult {
    match msg {
        ExecuteMsg::Swap {
            min_return,
            ..
        } => {
            let mut response = Response::new();
            if let Some(min_return) = min_return {
                for coin in min_return {
                    let balance = native_asset_info(coin.denom.clone())
                        .query_pool(&deps.querier, env.contract.address.to_string())?;

                    response = response.add_message(
                        native_asset_info(coin.denom.to_string())
                            .with_balance(balance)
                            .transfer_msg(&info.sender)?
                            .to_normal()?,
                    )
                }
            }

            Ok(response)
        },
    }
}

#[entry_point]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    Err(StdError::generic_err("not supported"))
}
