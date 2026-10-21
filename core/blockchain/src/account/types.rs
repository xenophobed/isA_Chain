use crate::types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
