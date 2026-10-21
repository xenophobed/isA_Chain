use crate::types::{Address, Amount, BlockHeight};

/// Errors related to treasury operations
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TreasuryError {
    #[error("Insufficient treasury balance to distribute")]
    InsufficientBalance,

    #[error("Invalid fee rate: {0} bps exceeds maximum of 10000 bps")]
    InvalidFeeRate(u32),

    #[error("Amount must be greater than zero")]
    ZeroAmount,

    #[error("Distribution requires at least one recipient")]
    NoRecipients,

    #[error("Unauthorized admin operation: {0} is not the admin")]
    UnauthorizedAdmin(Address),
}

/// A record of a single distribution event from the treasury to stakers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Distribution {
    /// Who received funds and how much each recipient got
    pub recipients: Vec<(Address, Amount)>,
    /// Total amount distributed in this event
    pub total_amount: Amount,
    /// Block height at which the distribution occurred
    pub height: BlockHeight,
}
