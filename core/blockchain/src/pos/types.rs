use crate::types::constants::VALIDATOR_MIN_STAKE;
use crate::types::{Address, Amount};

/// Errors that can occur in the PoS consensus engine.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PoSError {
    #[error("Insufficient validators: need at least {0}")]
    InsufficientValidators(usize),

    #[error("Invalid proposer: {0}")]
    InvalidProposer(Address),

    #[error("Insufficient voting power: required {required}, actual {actual}")]
    InsufficientVotingPower { required: u64, actual: u64 },

    #[error("Validator not found: {0}")]
    ValidatorNotFound(Address),

    #[error("Duplicate validator: {0}")]
    DuplicateValidator(Address),

    #[error("Staking error: {0}")]
    StakingError(String),
}

/// Metadata tracked per active validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorInfo {
    /// On-chain address of the validator
    pub address: Address,
    /// Active stake amount (in base ISA units)
    pub stake: Amount,
    /// Voting weight — proportional to stake, scaled to u64
    pub weight: u64,
    /// Total blocks this validator has successfully proposed
    pub blocks_proposed: u64,
    /// Total blocks this validator was selected but missed
    pub blocks_missed: u64,
}

impl ValidatorInfo {
    pub(crate) fn new(address: Address, stake: Amount) -> Self {
        ValidatorInfo {
            address,
            stake,
            weight: stake_to_weight(stake),
            blocks_proposed: 0,
            blocks_missed: 0,
        }
    }
}

/// Convert a raw stake amount to a u64 voting weight.
///
/// We divide by VALIDATOR_MIN_STAKE so that one minimum-stake unit = 1 weight.
/// This keeps the numbers small enough to avoid u64 overflow in
/// cumulative-weight arithmetic while preserving proportionality.
pub(crate) fn stake_to_weight(stake: Amount) -> u64 {
    // Guard against zero min-stake (shouldn't happen in production)
    if VALIDATOR_MIN_STAKE == 0 {
        return stake as u64;
    }
    // Safe: VALIDATOR_MIN_STAKE is 32_000 ISA in its base unit;
    // even with 1B tokens staked the result fits comfortably in u64.
    (stake / VALIDATOR_MIN_STAKE) as u64
}
