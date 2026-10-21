use crate::types::{Address, Amount, BlockHeight};
use serde::{Deserialize, Serialize};

// ============================================================================
// VeISAError
// ============================================================================

/// Errors related to vote-escrowed ISA operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum VeISAError {
    #[error("Insufficient amount: minimum required is not met")]
    InsufficientAmount,

    #[error("Lock has not yet expired; cannot withdraw before lock_end")]
    LockNotExpired,

    #[error("No lock found for address {0:?}")]
    NoLockFound(Address),

    #[error("Lock already exists for address {0:?}; use extend_lock or increase_amount")]
    LockAlreadyExists(Address),

    #[error("Invalid lock duration: must be between min and max lock duration")]
    InvalidDuration,

    #[error("Amount must be greater than zero")]
    ZeroAmount,
}

// ============================================================================
// VeISALock
// ============================================================================

/// A single vote-escrowed ISA lock record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VeISALock {
    /// The owner of this lock.
    pub owner: Address,
    /// Amount of ISA locked.
    pub amount: Amount,
    /// Block height when the lock was created.
    pub lock_start: BlockHeight,
    /// Block height when the lock expires.
    pub lock_end: BlockHeight,
    /// Voting power at the time of the last calculation.
    pub voting_power: Amount,
    /// Block height at which voting_power was last calculated.
    pub last_calculated: BlockHeight,
}
