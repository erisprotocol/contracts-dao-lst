use astroport::asset::{AssetInfo, AssetInfoExt};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    to_json_binary, CosmosMsg, DepsMut, Env, MessageInfo, Response, StdResult, WasmMsg,
};
use eris::{adapters::asset::AssetEx, hub::ClaimType};
use eris_chain_adapter::types::{CustomMsgType, CustomQueryType};

use crate::{
    error::{ContractError, ContractResult},
    state::State,
};

#[cw_serde]
pub enum ClaimExecuteMsg {
    Claim {},
}

impl ClaimExecuteMsg {
    pub fn into_msg(&self, contract_addr: String) -> StdResult<CosmosMsg<CustomMsgType>> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg: to_json_binary(&self)?,
            funds: vec![],
        }))
    }
}

#[cw_serde]
pub enum ClaimExecuteGenieMsg {
    Claim {
        payload: String,
    },
}

impl ClaimExecuteGenieMsg {
    pub fn into_msg(&self, contract_addr: String) -> StdResult<CosmosMsg<CustomMsgType>> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg: to_json_binary(&self)?,
            funds: vec![],
        }))
    }
}

pub fn exec_claim(
    deps: DepsMut<CustomQueryType>,
    env: Env,
    info: MessageInfo,
    claims: Vec<ClaimType>,
) -> ContractResult {
    let state = State::default();
    state.assert_owner(deps.storage, &info.sender)?;

    if claims.is_empty() {
        return Err(ContractError::NoClaimsProvided {});
    }

    let claim_msgs = claims
        .into_iter()
        .map(|claim| {
            Ok(match claim {
                ClaimType::Default(contract_addr) => {
                    deps.api.addr_validate(&contract_addr)?;
                    ClaimExecuteMsg::Claim {}.into_msg(contract_addr)?
                },
                ClaimType::Genie {
                    contract,
                    payload,
                } => {
                    deps.api.addr_validate(&contract)?;
                    ClaimExecuteGenieMsg::Claim {
                        payload,
                    }
                    .into_msg(contract)?
                },
                ClaimType::Transfer {
                    token,
                    recipient,
                } => {
                    let stake = state.stake_token.load(deps.storage)?;

                    let stake_denom = AssetInfo::NativeToken {
                        denom: stake.denom,
                    };

                    if stake.utoken == token || stake_denom == token {
                        return Err(ContractError::SwapFromNotAllowed(token.to_string()));
                    }

                    let balance = token.query_pool(&deps.querier, env.contract.address.clone())?;
                    let recipient = deps.api.addr_validate(&recipient)?;

                    token.with_balance(balance).transfer_msg(&recipient)?
                },
            })
        })
        .collect::<Result<Vec<CosmosMsg<CustomMsgType>>, ContractError>>()?;

    Ok(Response::new().add_messages(claim_msgs).add_attribute("action", "erishub/exec_claim"))
}
