use crate::types::{Address, Amount};
use serde::{Deserialize, Serialize};

/// Categories for genesis token allocations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllocationPurpose {
    Team,
    Treasury,
    Ecosystem,
    Validators,
    ProviderIncentives,
    EarlySupporter,
}

/// A single recipient entry in the genesis distribution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenesisAllocation {
    pub address: Address,
    pub amount: Amount,
    pub purpose: AllocationPurpose,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GenesisError {
    #[error("Total allocations exceed initial supply")]
    AllocationExceedsSupply,

    #[error("Duplicate allocation for address {0}")]
    DuplicateAllocation(Address),

    #[error("Invalid vesting schedule: cliff > end or amount = 0")]
    InvalidVestingSchedule,

    #[error("Initial supply must be greater than zero")]
    ZeroSupply,
}
