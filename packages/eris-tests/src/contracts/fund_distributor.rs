use astroport::asset::{native_asset, native_asset_info, token_asset_info, AssetInfoExt};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    entry_point, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
};

pub type ContractResult = Result<Response, StdError>;

#[cw_serde]
pub enum ExecuteMsg {
    ClaimRewards(ClaimRewardsMsg),
}

#[cw_serde]
pub struct ClaimRewardsMsg {
    pub user: String,
    /// Native denominations to be claimed
    pub native_denoms: Option<Vec<String>>,
    /// CW20 asset rewards to be claimed, should be addresses of CW20 tokens
    pub cw20_assets: Option<Vec<String>>,
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
        ExecuteMsg::ClaimRewards(claim) => {
            let mut response = Response::new();

            if let Some(cw20_assets) = claim.cw20_assets {
                for cw20 in cw20_assets {
                    let asset_info = token_asset_info(Addr::unchecked(cw20));
                    let balance =
                        asset_info.query_pool(&deps.querier, env.contract.address.clone())?;

                    if !balance.is_zero() {
                        response = response.add_message(
                            asset_info.with_balance(balance).into_msg(info.sender.clone())?,
                        )
                    }
                }
            }
            if let Some(native_denoms) = claim.native_denoms {
                if native_denoms.is_empty() {
                    let balances = deps.querier.query_all_balances(env.contract.address)?;
                    println!("send funds {:?}", balances);
                    for coin in balances {
                        response = response.add_message(
                            native_asset(coin.denom, coin.amount).into_msg(info.sender.clone())?,
                        )
                    }
                } else {
                    for denom in native_denoms {
                        let asset_info = native_asset_info(denom);
                        let balance =
                            asset_info.query_pool(&deps.querier, env.contract.address.clone())?;

                        if !balance.is_zero() {
                            response = response.add_message(
                                asset_info.with_balance(balance).into_msg(info.sender.clone())?,
                            )
                        }
                    }
                }
            } else {
                let balances = deps.querier.query_all_balances(env.contract.address)?;
                println!("send funds {:?}", balances);
                for coin in balances {
                    response = response.add_message(
                        native_asset(coin.denom, coin.amount).into_msg(info.sender.clone())?,
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
