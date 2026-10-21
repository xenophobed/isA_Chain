use crate::subnet::SubnetId;
use crate::types::{Address, Amount, BlockHeight};
use serde::{Deserialize, Serialize};

/// A single pending unbonding chunk for a subnet stake.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubnetUnbonding {
    pub amount: Amount,
    pub completion_height: BlockHeight,
}

/// Per-(subnet, provider) stake record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubnetStake {
    pub provider: Address,
    pub subnet_id: SubnetId,
    pub amount: Amount,
    pub staked_at: BlockHeight,
    pub unbonding: Vec<SubnetUnbonding>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SubnetStakingError {
    #[error("Insufficient stake: required {required}, provided {provided}")]
    InsufficientStake { required: Amount, provided: Amount },

    #[error("No stake found for this provider/subnet combination")]
    NoStakeFound,

    #[error("Subnet not found")]
    SubnetNotFound,

    #[error("Insufficient balance for requested operation")]
    InsufficientBalance,

    #[error("Provider already has a stake in this subnet; use add_stake to increase")]
    AlreadyStaked,
}
