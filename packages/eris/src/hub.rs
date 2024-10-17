use astroport::asset::{Asset, AssetInfo};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{
    to_json_binary, Addr, Api, CosmosMsg, Decimal, Empty, StdResult, Uint128, VoteOption, WasmMsg,
};
use cw20::Cw20ReceiveMsg;
use eris_chain_adapter::types::{
    CustomMsgType, DenomType, MultiSwapRouterType, StageType, WithdrawType,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// StageType = DEX
// DenomType = Chain specific denom
// Option<Decimal> = Price
// Option<Uint128> = max amount, 0 = unlimited
pub type SingleSwapConfig = (StageType, DenomType, Option<Decimal>, Option<Uint128>);

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
    /// How often the unbonding queue is to be executed, in seconds
    pub epoch_period: u64,
    /// The staking module's unbonding time, in seconds
    pub unbond_period: u64,

    /// Contract address where fees are sent
    pub protocol_fee_contract: String,
    /// Fees that are being applied during reinvest of staking rewards
    pub protocol_reward_fee: Decimal, // "1 is 100%, 0.05 is 5%"
    /// Contract address that is allowed to vote
    pub vote_operator: Option<String>,

    /// Dao specific config
    pub dao_interface: DaoInterface<String>,
}

#[cw_serde]
pub enum DaoInterface<T> {
    Enterprise {
        addr: T,
        fund_distributor: T,
    },
    EnterpriseV2 {
        gov: T,
        membership: T,
        distributor: T,
    },
    Cw4 {
        // calling bond, unbond, claim  (CW4)
        addr: T,
        // calling vote (CW3)
        gov: T,
        // calling claimrewards
        fund_distributor: T,
    },
    DaoDao {
        // calling bond, unbond, claim  (CW4)
        staking: T,
        // calling vote (CW3)
        gov: T,
        /// entropic variant of rewards claimable
        cw_rewards: T,
    },
    //
    DaoDaoV2 {
        // calling bond, unbond, claim  (CW4)
        staking: T,
        // calling vote (CW3)
        gov: T,
        // calling claim with id on each of the contracts
        rewards: Vec<(T, u64)>,
    },
    Alliance {
        addr: T,
    },
    Capa {
        gov: T,
    },
}

impl DaoInterface<String> {
    pub fn validate(&self, api: &dyn Api) -> StdResult<DaoInterface<Addr>> {
        Ok(match self {
            DaoInterface::Enterprise {
                addr,
                fund_distributor,
            } => DaoInterface::Enterprise {
                addr: api.addr_validate(addr)?,
                fund_distributor: api.addr_validate(fund_distributor)?,
            },
            DaoInterface::EnterpriseV2 {
                distributor,
                gov,
                membership,
            } => DaoInterface::EnterpriseV2 {
                distributor: api.addr_validate(distributor)?,
                gov: api.addr_validate(gov)?,
                membership: api.addr_validate(membership)?,
            },
            DaoInterface::Cw4 {
                addr,
                gov,
                fund_distributor,
            } => DaoInterface::Cw4 {
                addr: api.addr_validate(addr)?,
                gov: api.addr_validate(gov)?,
                fund_distributor: api.addr_validate(fund_distributor)?,
            },
            DaoInterface::Alliance {
                addr,
            } => DaoInterface::Alliance {
                addr: api.addr_validate(addr)?,
            },
            DaoInterface::Capa {
                gov,
            } => DaoInterface::Capa {
                gov: api.addr_validate(gov)?,
            },
            DaoInterface::DaoDao {
                staking: addr,
                gov,
                cw_rewards: fund_distributor,
            } => DaoInterface::DaoDao {
                staking: api.addr_validate(addr)?,
                gov: api.addr_validate(gov)?,
                cw_rewards: api.addr_validate(fund_distributor)?,
            },
            DaoInterface::DaoDaoV2 {
                staking: addr,
                gov,
                rewards: fund_distributor,
            } => DaoInterface::DaoDaoV2 {
                staking: api.addr_validate(addr)?,
                gov: api.addr_validate(gov)?,
                rewards: fund_distributor
                    .clone()
                    .into_iter()
                    .map(|(contract, claim_id)| Ok((api.addr_validate(&contract)?, claim_id)))
                    .collect::<StdResult<Vec<_>>>()?,
            },
        })
    }
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
    /// Withdraw Token that have finished unbonding in previous batches
    WithdrawUnbonded {
        receiver: Option<String>,
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

    /// Update Token amounts in unbonding batches to reflect any slashing or rounding errors
    Reconcile {},
    /// Submit the current pending batch of unbonding requests to be unbonded
    SubmitBatch {},
    /// Vote on a proposal (only allowed by the vote_operator)
    Vote {
        proposal_id: u64,
        vote: VoteOption,
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

        /// Update the vote_operator
        vote_operator: Option<String>,
        /// Update the default max_spread
        default_max_spread: Option<u64>,

        /// How often the unbonding queue is to be executed, in seconds
        epoch_period: Option<u64>,
        /// The staking module's unbonding time, in seconds
        unbond_period: Option<u64>,

        /// Update the DAO config
        dao_interface: Option<DaoInterface<String>>,
    },

    /// Submit an unbonding request to the current unbonding queue; automatically invokes `unbond`
    /// if `epoch_time` has elapsed since when the last unbonding queue was executed.
    QueueUnbond {
        receiver: Option<String>,
    },

    // Claim possible airdrops
    Claim {
        claims: Vec<ClaimType>,
    },
}

#[cw_serde]
pub enum ReceiveMsg {
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
    },
    MultiSwapRouter {
        router: MultiSwapRouter,
    },
    /// Following the swaps, stake the Token acquired to the whitelisted validators
    Reinvest {},

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
            msg: to_json_binary(&ExecuteMsg::Callback(self.clone()))?,
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
    /// The current batch on unbonding requests pending submission. Response: `PendingBatch`
    #[returns(PendingBatch)]
    PendingBatch {},
    /// Query an individual batch that has previously been submitted for unbonding but have not yet
    /// fully withdrawn. Response: `Batch`
    #[returns(Batch)]
    PreviousBatch(u64),
    /// Enumerate all previous batches that have previously been submitted for unbonding but have not
    /// yet fully withdrawn. Response: `Vec<Batch>`
    #[returns(Vec<Batch>)]
    PreviousBatches {
        start_after: Option<u64>,
        limit: Option<u32>,
    },
    /// Enumerate all outstanding unbonding requests in a given batch. Response: `Vec<UnbondRequestsByBatchResponseItem>`
    #[returns(Vec<UnbondRequestsByBatchResponseItem>)]
    UnbondRequestsByBatch {
        id: u64,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Enumreate all outstanding unbonding requests from given a user. Response: `Vec<UnbondRequestsByUserResponseItem>`
    #[returns(Vec<UnbondRequestsByUserResponseItem>)]
    UnbondRequestsByUser {
        user: String,
        start_after: Option<u64>,
        limit: Option<u32>,
    },
    /// Enumreate all outstanding unbonding requests from given a user. Response: `Vec<UnbondRequestsByUserResponseItemDetails>`
    #[returns(Vec<UnbondRequestsByUserResponseItemDetails>)]
    UnbondRequestsByUserDetails {
        user: String,
        start_after: Option<u64>,
        limit: Option<u32>,
    },

    #[returns(ExchangeRatesResponse)]
    ExchangeRates {
        // start after the provided timestamp in s
        start_after: Option<u64>,
        limit: Option<u32>,
    },
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

    /// How often the unbonding queue is to be executed, in seconds
    pub epoch_period: u64,
    /// The staking module's unbonding time, in seconds
    pub unbond_period: u64,

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

    /// Update the vote_operator
    pub vote_operator: Option<String>,

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
    // Amount of utoken currently unbonding
    pub unbonding: Uint128,
    // Amount of utoken currently available as balance of the contract
    pub available: Uint128,
    // Total amount of utoken within the contract (bonded + unbonding + available)
    pub tvl_utoken: Uint128,
}

#[cw_serde]
pub struct PendingBatch {
    /// ID of this batch
    pub id: u64,
    /// Total amount of `ustake` to be burned in this batch
    pub ustake_to_burn: Uint128,
    /// Estimated time when this batch will be submitted for unbonding
    pub est_unbond_start_time: u64,
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

    #[serde(default)]
    pub disabled: bool,
}

#[cw_serde]
pub struct FeeConfig {
    /// Contract address where fees are sent
    pub protocol_fee_contract: Addr,
    /// Fees that are being applied during reinvest of staking rewards
    pub protocol_reward_fee: Decimal, // "1 is 100%, 0.05 is 5%"
}

#[cw_serde]
pub struct Batch {
    /// ID of this batch
    pub id: u64,
    /// Whether this batch has already been reconciled
    pub reconciled: bool,
    /// Total amount of shares remaining this batch. Each `ustake` burned = 1 share
    pub total_shares: Uint128,
    /// Amount of `utoken` in this batch that have not been claimed
    pub utoken_unclaimed: Uint128,
    /// Estimated time when this batch will finish unbonding
    pub est_unbond_end_time: u64,
}

#[cw_serde]
pub struct UnbondRequest {
    /// ID of the batch
    pub id: u64,
    /// The user's address
    pub user: Addr,
    /// The user's share in the batch
    pub shares: Uint128,
}

#[cw_serde]
pub struct UnbondRequestsByBatchResponseItem {
    /// The user's address
    pub user: String,
    /// The user's share in the batch
    pub shares: Uint128,
}

impl From<UnbondRequest> for UnbondRequestsByBatchResponseItem {
    fn from(s: UnbondRequest) -> Self {
        Self {
            user: s.user.into(),
            shares: s.shares,
        }
    }
}

#[cw_serde]
pub struct UnbondRequestsByUserResponseItem {
    /// ID of the batch
    pub id: u64,
    /// The user's share in the batch
    pub shares: Uint128,
}

impl From<UnbondRequest> for UnbondRequestsByUserResponseItem {
    fn from(s: UnbondRequest) -> Self {
        Self {
            id: s.id,
            shares: s.shares,
        }
    }
}

#[cw_serde]
pub struct UnbondRequestsByUserResponseItemDetails {
    /// ID of the batch
    pub id: u64,
    /// The user's share in the batch
    pub shares: Uint128,

    // state of pending, unbonding or completed
    pub state: String,

    // The details of the unbonding batch
    pub batch: Option<Batch>,

    // Is set if the unbonding request is still pending
    pub pending: Option<PendingBatch>,
}

#[cw_serde]
pub struct ExchangeRatesResponse {
    pub exchange_rates: Vec<(u64, Decimal)>,
    // APR normalized per DAY
    pub apr: Option<Decimal>,
}

#[cw_serde]
pub enum ClaimType {
    Default(String),
    Genie {
        contract: String,
        payload: String,
    },
    Transfer {
        token: AssetInfo,
        recipient: String,
    },
}

#[cw_serde]
pub struct MigrateMsg {
    pub action: Option<MigrateAction>,
}

#[cw_serde]
pub enum MigrateAction {
    Disable,
    Unstake,
    Claim,
    ReconcileAll,
    Stake,
    Enable,
}
