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

/// Different types of accounts
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AccountType {
    /// Regular user account (externally owned account)
    External,
    /// Smart contract account
    Contract {
        /// Contract creation block height
        created_at: BlockHeight,
        /// Contract creator address
        creator: Address,
    },
    /// System account (for built-in functionality)
    System {
        /// System account purpose
        purpose: SystemAccountPurpose,
    },
}

/// System account purposes
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SystemAccountPurpose {
    /// Validator rewards distribution
    ValidatorRewards,
    /// Treasury for governance
    Treasury,
    /// Bridge escrow
    BridgeEscrow,
    /// Fee collection
    FeeCollection,
}

/// Staking information for validators and delegators
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StakingInfo {
    /// Validator information (if this account is a validator)
    pub validator: Option<ValidatorAccount>,
    
    /// Delegations made by this account
    pub delegations: HashMap<Address, Delegation>,
    
    /// Total amount staked (self + delegated to others)
    pub total_staked: Amount,
    
    /// Rewards accumulated but not yet claimed
    pub pending_rewards: Amount,
    
    /// Unbonding delegations
    pub unbonding: Vec<UnbondingDelegation>,
}

/// Validator account information
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ValidatorAccount {
    /// Validator public key
    pub public_key: Vec<u8>,
    
    /// Commission rate (basis points, 10000 = 100%)
    pub commission_rate: u32,
    
    /// Current commission rewards
    pub commission_rewards: Amount,
    
    /// Minimum self delegation amount
    pub min_self_delegation: Amount,
    
    /// Current self delegation
    pub self_delegation: Amount,
    
    /// Total delegation power (self + others)
    pub total_delegation: Amount,
    
    /// Validator status
    pub status: ValidatorStatus,
    
    /// Validator description
    pub description: ValidatorDescription,
    
    /// Number of blocks signed in recent window
    pub signed_blocks: u32,
    
    /// Number of blocks missed in recent window
    pub missed_blocks: u32,
    
    /// Last block height when validator was active
    pub last_active_height: BlockHeight,
    
    /// Accumulated slashing penalties
    pub total_slashed: Amount,
}

/// Validator status
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ValidatorStatus {
    /// Active validator participating in consensus
    Active,
    /// Inactive validator (not participating)
    Inactive,
    /// Jailed validator (temporarily suspended)
    Jailed {
        /// Block height when jailed
        jailed_at: BlockHeight,
        /// Reason for jailing
        reason: JailReason,
        /// Minimum height when can unjail
        unjail_at: BlockHeight,
    },
    /// Permanently slashed validator
    Slashed {
        /// Block height when slashed
        slashed_at: BlockHeight,
        /// Slashing reason
        reason: SlashReason,
    },
}

/// Reasons for jailing a validator
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum JailReason {
    /// Too many missed blocks
    Downtime,
    /// Double signing detected
    DoubleSign,
    /// Invalid proposal
    InvalidProposal,
}

/// Reasons for slashing a validator
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SlashReason {
    /// Severe double signing
    DoubleSign,
    /// Malicious behavior
    MaliciousBehavior,
    /// Consensus violation
    ConsensusViolation,
}

/// Delegation information
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Delegation {
    /// Amount delegated
    pub amount: Amount,
    
    /// Block height when delegation was made
    pub delegated_at: BlockHeight,
    
    /// Accumulated rewards from this delegation
    pub rewards: Amount,
    
    /// Last height when rewards were calculated
    pub last_reward_height: BlockHeight,
}

/// Unbonding delegation (withdrawal in progress)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct UnbondingDelegation {
    /// Validator being undelegated from
    pub validator: Address,
    
    /// Amount being unbonded
    pub amount: Amount,
    
    /// Block height when unbonding started
    pub unbonding_height: BlockHeight,
    
    /// Block height when unbonding completes
    pub completion_height: BlockHeight,
}

/// Validator description from transaction
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ValidatorDescription {
    pub moniker: String,
    pub identity: String,
    pub website: String,
    pub security_contact: String,
    pub details: String,
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
            account_type: AccountType::Contract { created_at, creator },
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
        matches!(
            self.validator_status(),
            Some(ValidatorStatus::Active)
        )
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
        
        let delegation = staking_info.delegations.entry(validator).or_insert(Delegation {
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
        let staking_info = self.staking_info
            .as_mut()
            .ok_or(AccountError::NoStakingInfo)?;
        
        let delegation = staking_info.delegations
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
        let serialized = bincode::serialize(self)
            .expect("Account serialization should never fail");
        Hash::hash_data(&serialized)
    }
}

/// Account-related errors
#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("Insufficient balance: available {available}, required {required}")]
    InsufficientBalance { available: Amount, required: Amount },
    
    #[error("Account is already a validator")]
    AlreadyValidator,
    
    #[error("Insufficient self delegation: minimum {minimum}, actual {actual}")]
    InsufficientSelfDelegation { minimum: Amount, actual: Amount },
    
    #[error("No staking information available")]
    NoStakingInfo,
    
    #[error("No delegation to validator {validator}")]
    NoDelegation { validator: Address },
    
    #[error("Insufficient delegation: available {available}, required {required}")]
    InsufficientDelegation { available: Amount, required: Amount },
    
    #[error("Account is jailed")]
    ValidatorJailed,
    
    #[error("Account is slashed")]
    ValidatorSlashed,
    
    #[error("Invalid commission rate")]
    InvalidCommissionRate,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_account_creation() {
        let account = Account::new_external(1000);
        
        assert_eq!(account.balance, 1000);
        assert_eq!(account.nonce, 0);
        assert!(!account.is_contract());
        assert!(!account.is_validator());
    }
    
    #[test]
    fn test_balance_operations() {
        let mut account = Account::new_external(1000);
        
        // Test successful deduction
        assert!(account.deduct_balance(500).is_ok());
        assert_eq!(account.balance, 500);
        
        // Test insufficient balance
        assert!(account.deduct_balance(600).is_err());
        
        // Test adding balance
        account.add_balance(200);
        assert_eq!(account.balance, 700);
    }
    
    #[test]
    fn test_validator_creation() {
        let mut account = Account::new_external(50000);
        
        let description = ValidatorDescription {
            moniker: "test-validator".to_string(),
            identity: "".to_string(),
            website: "".to_string(),
            security_contact: "".to_string(),
            details: "".to_string(),
        };
        
        assert!(account.become_validator(
            vec![1u8; 33],
            1000, // 10% commission
            32000,
            32000,
            description,
            0
        ).is_ok());
        
        assert!(account.is_validator());
        assert!(account.is_active_validator());
    }
    
    #[test]
    fn test_delegation() {
        let mut account = Account::new_external(1000);
        let validator = Address::from([1u8; 20]);
        
        assert!(account.delegate_to_validator(validator, 500, 0).is_ok());
        
        let staking_info = account.staking_info.as_ref().unwrap();
        assert_eq!(staking_info.total_staked, 500);
        assert!(staking_info.delegations.contains_key(&validator));
    }
}