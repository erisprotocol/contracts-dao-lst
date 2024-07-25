use astroport::asset::{Asset, AssetInfo};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{coin, to_json_binary, Addr, CosmosMsg, QuerierWrapper, StdResult, WasmMsg};
use cw20::Cw20ExecuteMsg;
use eris_chain_adapter::types::CustomMsgType;

use crate::hub_alliance::{ConfigResponse, ExecuteMsg, QueryMsg};

#[cw_serde]
pub struct AllianceHub(pub Addr);

impl AllianceHub {
    pub fn bond_msg(
        &self,
        asset: Asset,
        receiver: Option<String>,
        donate: Option<bool>,
    ) -> StdResult<CosmosMsg<CustomMsgType>> {
        match asset.info {
            AssetInfo::Token {
                contract_addr,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                funds: vec![],
                msg: to_json_binary(&Cw20ExecuteMsg::Send {
                    contract: self.0.to_string(),
                    amount: asset.amount,
                    msg: to_json_binary(&ExecuteMsg::Bond {
                        receiver,
                        donate,
                    })?,
                })?,
            })),
            AssetInfo::NativeToken {
                denom,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: self.0.to_string(),
                msg: to_json_binary(&ExecuteMsg::Bond {
                    receiver,
                    donate,
                })?,
                funds: vec![coin(asset.amount.u128(), denom)],
            })),
        }
    }

    pub fn unbond_msg(
        &self,
        denom: impl Into<String>,
        amount: u128,
        receiver: Option<String>,
    ) -> StdResult<CosmosMsg<CustomMsgType>> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: self.0.to_string(),
            msg: to_json_binary(&ExecuteMsg::Unbond {
                receiver,
            })?,
            funds: vec![coin(amount, denom)],
        }))
    }

    pub fn query_config(&self, querier: &QuerierWrapper) -> StdResult<ConfigResponse> {
        let config: ConfigResponse =
            querier.query_wasm_smart(self.0.to_string(), &QueryMsg::Config {})?;
        Ok(config)
    }
}
