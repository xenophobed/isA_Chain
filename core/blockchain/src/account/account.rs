use super::types::*;
use crate::types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Account state in the blockchain
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Account {
    /// Current account balance
    pub balance: Amount,

    /// Transaction nonce (prevents replay attacks)
    pub nonce: u64,

    /// Storage root for smart contract accounts
    pub storage_root: Hash,

    /// Code hash for smart contract accounts
    pub code_hash: Hash,

    /// Account type
    pub account_type: AccountType,

    /// Staking information (for validators and delegators)
    pub staking_info: Option<StakingInfo>,
}

impl Account {
    /// Create a new external account
    pub fn new_external(balance: Amount) -> Self {
        Account {
            balance,
            nonce: 0,
            storage_root: Hash::ZERO,
            code_hash: Hash::ZERO,
            account_type: AccountType::External,
            staking_info: None,
        }
    }

    /// Create a new contract account
    pub fn new_contract(
        balance: Amount,
        code_hash: Hash,
        storage_root: Hash,
        created_at: BlockHeight,
        creator: Address,
    ) -> Self {
        Account {
            balance,
            nonce: 0,
            storage_root,
            code_hash,
            account_type: AccountType::Contract {
                created_at,
                creator,
            },
            staking_info: None,
        }
    }

    /// Create a new system account
    pub fn new_system(purpose: SystemAccountPurpose, balance: Amount) -> Self {
        Account {
            balance,
            nonce: 0,
            storage_root: Hash::ZERO,
            code_hash: Hash::ZERO,
            account_type: AccountType::System { purpose },
            staking_info: None,
        }
    }

    /// Check if account is a smart contract
    pub fn is_contract(&self) -> bool {
        matches!(self.account_type, AccountType::Contract { .. })
    }

    /// Check if account is a system account
    pub fn is_system(&self) -> bool {
        matches!(self.account_type, AccountType::System { .. })
    }

    /// Check if account is a validator
    pub fn is_validator(&self) -> bool {
        self.staking_info
            .as_ref()
            .map(|info| info.validator.is_some())
            .unwrap_or(false)
    }

    /// Get validator status if this account is a validator
    pub fn validator_status(&self) -> Option<&ValidatorStatus> {
        self.staking_info
            .as_ref()?
            .validator
            .as_ref()
            .map(|v| &v.status)
    }

    /// Check if validator is active
    pub fn is_active_validator(&self) -> bool {
        matches!(self.validator_status(), Some(ValidatorStatus::Active))
    }

    /// Deduct amount from account balance
    pub fn deduct_balance(&mut self, amount: Amount) -> Result<(), AccountError> {
        if self.balance < amount {
            return Err(AccountError::InsufficientBalance {
                available: self.balance,
                required: amount,
            });
        }

        self.balance -= amount;
        Ok(())
    }

    /// Add amount to account balance
    pub fn add_balance(&mut self, amount: Amount) {
        self.balance += amount;
    }

    /// Increment account nonce
    pub fn increment_nonce(&mut self) {
        self.nonce += 1;
    }

    /// Become a validator
    pub fn become_validator(
        &mut self,
        public_key: Vec<u8>,
        commission_rate: u32,
        min_self_delegation: Amount,
        self_delegation: Amount,
        description: ValidatorDescription,
        height: BlockHeight,
    ) -> Result<(), AccountError> {
        if self.is_validator() {
            return Err(AccountError::AlreadyValidator);
        }

        if self_delegation < min_self_delegation {
            return Err(AccountError::InsufficientSelfDelegation {
                minimum: min_self_delegation,
                actual: self_delegation,
            });
        }

        let validator = ValidatorAccount {
            public_key,
            commission_rate,
            commission_rewards: 0,
            min_self_delegation,
            self_delegation,
            total_delegation: self_delegation,
            status: ValidatorStatus::Active,
            description,
            signed_blocks: 0,
            missed_blocks: 0,
            last_active_height: height,
            total_slashed: 0,
        };

        let staking_info = self.staking_info.get_or_insert_with(|| StakingInfo {
            validator: None,
            delegations: HashMap::new(),
            total_staked: 0,
            pending_rewards: 0,
            unbonding: Vec::new(),
        });

        staking_info.validator = Some(validator);
        staking_info.total_staked += self_delegation;

        Ok(())
    }

    /// Delegate to a validator
    pub fn delegate_to_validator(
        &mut self,
        validator: Address,
        amount: Amount,
        height: BlockHeight,
    ) -> Result<(), AccountError> {
        let staking_info = self.staking_info.get_or_insert_with(|| StakingInfo {
            validator: None,
            delegations: HashMap::new(),
            total_staked: 0,
            pending_rewards: 0,
            unbonding: Vec::new(),
        });

        let delegation = staking_info
            .delegations
            .entry(validator)
            .or_insert(Delegation {
                amount: 0,
                delegated_at: height,
                rewards: 0,
                last_reward_height: height,
            });

        delegation.amount += amount;
        staking_info.total_staked += amount;

        Ok(())
    }

    /// Undelegate from a validator
    pub fn undelegate_from_validator(
        &mut self,
        validator: Address,
        amount: Amount,
        height: BlockHeight,
        unbonding_period: u64,
    ) -> Result<(), AccountError> {
        let staking_info = self
            .staking_info
            .as_mut()
            .ok_or(AccountError::NoStakingInfo)?;

        let delegation = staking_info
            .delegations
            .get_mut(&validator)
            .ok_or(AccountError::NoDelegation { validator })?;

        if delegation.amount < amount {
            return Err(AccountError::InsufficientDelegation {
                available: delegation.amount,
                required: amount,
            });
        }

        delegation.amount -= amount;
        staking_info.total_staked -= amount;

        // Add to unbonding queue
        let unbonding = UnbondingDelegation {
            validator,
            amount,
            unbonding_height: height,
            completion_height: height + unbonding_period,
        };
        staking_info.unbonding.push(unbonding);

        // Remove delegation entry if amount is zero
        if delegation.amount == 0 {
            staking_info.delegations.remove(&validator);
        }

        Ok(())
    }

    /// Process completed unbonding delegations
    pub fn process_unbonding(&mut self, current_height: BlockHeight) -> Amount {
        let staking_info = match self.staking_info.as_mut() {
            Some(info) => info,
            None => return 0,
        };

        let mut total_unbonded = 0;
        staking_info.unbonding.retain(|unbonding| {
            if current_height >= unbonding.completion_height {
                total_unbonded += unbonding.amount;
                false // Remove from unbonding queue
            } else {
                true // Keep in queue
            }
        });

        // Add unbonded amount back to balance
        self.balance += total_unbonded;

        total_unbonded
    }

    /// Calculate account hash for state root
    pub fn hash(&self) -> Hash {
        let serialized =
            bincode::serialize(self).expect("Account serialization should never fail");
        Hash::hash_data(&serialized)
    }
}
