use astroport::asset::{Asset, AssetInfo};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{to_binary, Addr, CosmosMsg, Decimal, Empty, StdResult, Uint128, WasmMsg};
use cw20::Cw20ReceiveMsg;
use eris_chain_adapter::types::{
    CustomMsgType, DenomType, MultiSwapRouterType, StageType, WithdrawType,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::hub::DaoInterface;

// StageType = DEX
// DenomType = Chain specific denom
// Option<Decimal> = Price
// Option<Uint128> = max amount, 0 = unlimited
// Option<bool> = pay fee, 0 = no fee
pub type SingleSwapConfig = (StageType, DenomType, Option<Decimal>, Option<Uint128>, Option<bool>);

pub type MultiSwapRouter = (MultiSwapRouterType, Vec<DenomType>);

#[cw_serde]
pub struct InstantiateMsg {
    /// Account who can call certain privileged functions
    pub owner: String,
    /// Account who can call harvest
    pub operator: String,
    /// Denom of the underlaying staking token
    pub utoken: AssetInfo,
    /// Name of the liquid staking token
    pub denom: String,
    /// Contract address where fees are sent
    pub protocol_fee_contract: String,
    /// Fees that are being applied during reinvest of staking rewards
    pub protocol_reward_fee: Decimal, // "1 is 100%, 0.05 is 5%"
    /// Dao specific config
    pub dao_interface: DaoInterface<String>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Implements the Cw20 receiver interface
    Receive(Cw20ReceiveMsg),
    /// Bond specified amount of Token
    Bond {
        receiver: Option<String>,
        donate: Option<bool>,
    },

    /// Same as bond / unbond, belief_price, max_spread are ignored
    Swap {
        offer_asset: Asset,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
        to: Option<String>,
    },

    /// Transfer ownership to another account; will not take effect unless the new owner accepts
    TransferOwnership {
        new_owner: String,
    },
    /// Accept an ownership transfer
    AcceptOwnership {},
    /// Remove the ownership transfer proposal
    DropOwnershipProposal {},
    /// Claim staking rewards, swap all for Token, and restake
    Harvest {
        // specifies which validators should be harvested
        native_denoms: Option<Vec<String>>,
        cw20_assets: Option<Vec<String>>,
        withdrawals: Option<Vec<(WithdrawType, DenomType)>>,
        stages: Option<Vec<Vec<SingleSwapConfig>>>,
        router: Option<MultiSwapRouter>,
    },

    /// Callbacks; can only be invoked by the contract itself
    Callback(CallbackMsg),

    /// Updates the fee config,
    UpdateConfig {
        /// Contract address where fees are sent
        protocol_fee_contract: Option<String>,
        /// Fees that are being applied during reinvest of staking rewards
        protocol_reward_fee: Option<Decimal>, // "1 is 100%, 0.05 is 5%"
        /// Sets a new operator
        operator: Option<String>,
        /// Sets the stages preset
        stages_preset: Option<Vec<Vec<SingleSwapConfig>>>,
        /// Sets the withdrawals preset
        withdrawals_preset: Option<Vec<(WithdrawType, DenomType)>>,
        /// Specifies wether donations are allowed.
        allow_donations: Option<bool>,
        /// Update the default max_spread
        default_max_spread: Option<u64>,
    },

    /// Submit an unbonding request to the current unbonding queue; automatically invokes `unbond`
    /// if `epoch_time` has elapsed since when the last unbonding queue was executed.
    Unbond {
        receiver: Option<String>,
    },
}

#[cw_serde]
pub enum ReceiveMsg {
    /// Swap a given amount of asset
    Swap {
        ask_asset_info: Option<AssetInfo>,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
        to: Option<String>,
    },
    Bond {
        receiver: Option<String>,
        donate: Option<bool>,
    },
}

#[cw_serde]
pub enum CallbackMsg {
    WithdrawLps {
        withdrawals: Vec<(WithdrawType, DenomType)>,
    },
    // SingleStageSwap is executed multiple times to execute each swap stage. A stage consists of multiple swaps
    SingleStageSwap {
        // (Used dex, used denom, belief_price)
        stage: Vec<SingleSwapConfig>,
        index: usize,
    },
    MultiSwapRouter {
        router: MultiSwapRouter,
    },
    /// Following the swaps, stake the Token acquired to the whitelisted validators
    Reinvest {
        skip_fee: bool,
    },

    AssertBalance {
        expected: Asset,
    },

    CheckReceivedCoin {
        snapshot: Asset,
        snapshot_stake: Asset,
    },
}

impl CallbackMsg {
    pub fn into_cosmos_msg(&self, contract_addr: &Addr) -> StdResult<CosmosMsg<CustomMsgType>> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_addr.to_string(),
            msg: to_binary(&ExecuteMsg::Callback(self.clone()))?,
            funds: vec![],
        }))
    }
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// The contract's configurations. Response: `ConfigResponse`
    #[returns(ConfigResponse)]
    Config {},
    /// The contract's current state. Response: `StateResponse`
    #[returns(StateResponse)]
    State {},

    #[returns(ExchangeRatesResponse)]
    ExchangeRates {
        // start after the provided timestamp in s
        start_after: Option<u64>,
        limit: Option<u32>,
    },

    /// Returns information about a pair in an object of type [`super::asset::PairInfo`].
    #[returns(PairInfo)]
    Pair {},
}

/// This structure stores the main parameters for an Astroport pair
#[cw_serde]
pub struct PairInfo {
    /// Asset information for the assets in the pool
    pub asset_infos: Vec<AssetInfo>,
    /// Pair contract address
    pub contract_addr: Addr,
    /// Pair LP token address
    pub liquidity_token: Addr,
    /// The pool type (xyk, stableswap etc) available in [`PairType`]
    pub pair_type: PairType,
}

#[cw_serde]
pub enum PairType {
    /// XYK pair type
    Xyk {},
    /// Stable pair type
    Stable {},
    /// Custom pair type
    Custom(String),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    /// Account who can call certain privileged functions
    pub owner: String,
    /// Pending ownership transfer, awaiting acceptance by the new owner
    pub new_owner: Option<String>,
    /// Underlying staked token
    pub utoken: AssetInfo,
    /// Address of the Stake token
    pub stake_token: String,

    /// Information about applied fees
    pub fee_config: FeeConfig,

    /// Account who can call harvest
    pub operator: String,
    /// Stages that must be used by permissionless users
    pub stages_preset: Vec<Vec<SingleSwapConfig>>,
    /// withdrawals that must be used by permissionless users
    pub withdrawals_preset: Vec<(WithdrawType, DenomType)>,
    /// Specifies wether donations are allowed.
    pub allow_donations: bool,

    /// address of the DAO
    pub dao_interface: DaoInterface<Addr>,
}

#[cw_serde]
pub struct StateResponse {
    /// Total supply to the Stake token
    pub total_ustake: Uint128,
    /// Total amount of utoken staked (bonded)
    pub total_utoken: Uint128,
    /// The exchange rate between ustake and utoken, in terms of utoken per ustake
    pub exchange_rate: Decimal,
    /// Staking rewards currently held by the contract that are ready to be reinvested
    pub unlocked_coins: Vec<Asset>,
    // Amount of utoken currently available as balance of the contract
    pub available: Uint128,
    // Total amount of utoken within the contract (bonded + unbonding + available)
    pub tvl_utoken: Uint128,
}

#[cw_serde]
pub struct StakeToken {
    /// address of the DAO
    pub dao_interface: DaoInterface<Addr>,
    /// denom of the underlying token
    pub utoken: AssetInfo,
    // denom of the stake token
    pub denom: String,
    // amount of utoken bonded
    pub total_utoken_bonded: Uint128,
    // supply of the stake token
    pub total_supply: Uint128,
}

#[cw_serde]
pub struct FeeConfig {
    /// Contract address where fees are sent
    pub protocol_fee_contract: Addr,
    /// Fees that are being applied during reinvest of staking rewards
    pub protocol_reward_fee: Decimal, // "1 is 100%, 0.05 is 5%"
}

#[cw_serde]
pub struct ExchangeRatesResponse {
    pub exchange_rates: Vec<(u64, Decimal)>,
    // APR normalized per DAY
    pub apr: Option<Decimal>,
}

pub type MigrateMsg = Empty;
