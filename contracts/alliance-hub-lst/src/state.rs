use astroport::asset::Asset;
use cosmwasm_std::{Addr, Decimal, Storage};
use cw_storage_plus::{Item, Map};

use eris::hub_alliance::{FeeConfig, SingleSwapConfig, StakeToken};
use eris_chain_adapter::types::{DenomType, WithdrawType};
use serde::{de::DeserializeOwned, Serialize};

use crate::error::ContractError;

pub struct State<'a> {
    /// Account who can call certain privileged functions
    pub owner: Item<'a, Addr>,
    /// Account who can call harvest
    pub operator: Item<'a, Addr>,
    /// Account who can call vote
    pub vote_operator: Item<'a, Addr>,
    /// Stages that must be used by permissionless users
    pub stages_preset: Item<'a, Vec<Vec<SingleSwapConfig>>>,
    /// Withdraws that must be used by permissionless users
    pub withdrawals_preset: Item<'a, Vec<(WithdrawType, DenomType)>>,

    /// Pending ownership transfer, awaiting acceptance by the new owner
    pub new_owner: Item<'a, Addr>,
    /// Denom and supply of the Liquid Staking token
    pub stake_token: Item<'a, StakeToken>,
    /// Coins that can be reinvested
    pub unlocked_coins: Item<'a, Vec<Asset>>,

    /// Fee Config
    pub fee_config: Item<'a, FeeConfig>,
    /// Specifies wether the contract allows donations
    pub allow_donations: Item<'a, bool>,

    // history of the exchange_rate
    pub exchange_history: Map<'a, u64, Decimal>,

    pub default_max_spread: Item<'a, u64>,
}

impl Default for State<'static> {
    fn default() -> Self {
        Self {
            owner: Item::new("owner"),
            new_owner: Item::new("new_owner"),
            operator: Item::new("operator"),
            vote_operator: Item::new("vote_operator"),
            stages_preset: Item::new("stages_preset"),
            withdrawals_preset: Item::new("withdrawals_preset"),
            stake_token: Item::new("stake_token"),
            unlocked_coins: Item::new("unlocked_coins"),
            fee_config: Item::new("fee_config"),
            allow_donations: Item::new("allow_donations"),
            exchange_history: Map::new("exchange_history"),
            default_max_spread: Item::new("default_max_spread"),
        }
    }
}

impl<'a> State<'a> {
    pub fn assert_owner(&self, storage: &dyn Storage, sender: &Addr) -> Result<(), ContractError> {
        let owner = self.owner.load(storage)?;
        if *sender == owner {
            Ok(())
        } else {
            Err(ContractError::Unauthorized {})
        }
    }

    pub fn assert_operator(
        &self,
        storage: &dyn Storage,
        sender: &Addr,
    ) -> Result<(), ContractError> {
        let operator = self.operator.load(storage)?;
        if *sender == operator {
            Ok(())
        } else {
            Err(ContractError::UnauthorizedSenderNotOperator {})
        }
    }

    pub fn assert_vote_operator(
        &self,
        storage: &dyn Storage,
        sender: &Addr,
    ) -> Result<(), ContractError> {
        let vote_operator =
            self.vote_operator.load(storage).map_err(|_| ContractError::NoVoteOperatorSet {})?;

        if *sender == vote_operator {
            Ok(())
        } else {
            Err(ContractError::UnauthorizedSenderNotVoteOperator {})
        }
    }

    pub fn get_or_preset<T>(
        &self,
        storage: &dyn Storage,
        stages: Option<Vec<T>>,
        preset: &Item<'static, Vec<T>>,
        sender: &Addr,
    ) -> Result<Option<Vec<T>>, ContractError>
    where
        T: Serialize + DeserializeOwned,
    {
        let stages = if let Some(stages) = stages {
            if stages.is_empty() {
                None
            } else {
                // only operator is allowed to send custom stages. Otherwise the contract would be able to interact with "bad contracts"
                // to fully decentralize, it would be required, that there is a whitelist of withdraw and swap contracts in the contract or somewhere else
                self.assert_operator(storage, sender)?;
                Some(stages)
            }
        } else {
            // otherwise use configured stages
            preset.may_load(storage)?
        };
        Ok(stages)
    }

    pub fn get_default_max_spread(&self, storage: &dyn Storage) -> Decimal {
        // by default a max_spread of 10% is used.
        Decimal::percent(self.default_max_spread.load(storage).unwrap_or(10))
    }
}
