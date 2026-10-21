use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};

// ====================================================================
// Constants
// ====================================================================

/// Default withdrawal fee: 50 bps = 0.5%
pub const DEFAULT_WITHDRAWAL_FEE_BPS: u32 = 50;

/// Default cooldown period in blocks (~5 minutes at 3s/block)
pub const DEFAULT_COOLDOWN_BLOCKS: u64 = 100;

/// Default minimum withdrawal: 1_000 credits = $10.00
pub const DEFAULT_MIN_WITHDRAWAL: Amount = 1_000;

/// Default daily limit per user: 100_000_000 credits = $1,000,000
pub const DEFAULT_DAILY_LIMIT: Amount = 100_000_000;

/// Basis points denominator
pub(crate) const BPS_DENOMINATOR: Amount = 10_000;

// ====================================================================
// WithdrawalStatus
// ====================================================================

/// Lifecycle state of a withdrawal request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WithdrawalStatus {
    /// Request received, not yet in cooldown.
    Pending,
    /// Waiting for the cooldown period to elapse.
    InCooldown,
    /// Cooldown complete; being processed on-chain.
    Processing,
    /// ISA has been minted and the tx is confirmed.
    Completed,
    /// Processing failed; reason is stored in the variant.
    Failed(String),
    /// User cancelled before processing began.
    Cancelled,
}

// ====================================================================
// WithdrawalRequest
// ====================================================================

/// A single off-chain-credits → on-chain-ISA withdrawal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WithdrawalRequest {
    /// Unique identifier for this request.
    pub id: Hash,
    /// User who initiated the withdrawal.
    pub user: Address,
    /// Credits being withdrawn (before fee deduction).
    pub credits_amount: Amount,
    /// ISA tokens to be minted to the user.
    pub isa_to_receive: Amount,
    /// ISA/USD price (micro-USD) at withdrawal time.
    pub isa_price_usd: Amount,
    /// Withdrawal fee in credits.
    pub fee: Amount,
    /// Current lifecycle status.
    pub status: WithdrawalStatus,
    /// Block height when the request was created.
    pub created_at: BlockHeight,
    /// Block height when the request was completed (or failed/cancelled).
    pub completed_at: Option<BlockHeight>,
    /// On-chain mint transaction hash (set on completion).
    pub tx_hash: Option<Hash>,
    /// Block height at which the cooldown period ends.
    pub cooldown_ends: BlockHeight,
}

// ====================================================================
// WithdrawalError
// ====================================================================

/// Errors that can arise during withdrawal operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WithdrawalError {
    #[error("Insufficient credits for withdrawal")]
    InsufficientCredits,

    #[error("Invalid amount: must be greater than zero")]
    InvalidAmount,

    #[error("Withdrawal not found: {0:?}")]
    WithdrawalNotFound(Hash),

    #[error("Cooldown period has not yet completed")]
    CooldownNotComplete,

    #[error("Withdrawal has already been processed")]
    AlreadyProcessed,

    #[error("Below minimum withdrawal: minimum {minimum}, requested {requested}")]
    MinimumNotMet { minimum: Amount, requested: Amount },

    #[error("Daily limit exceeded: limit {limit}, already used {used}")]
    DailyLimitExceeded { limit: Amount, used: Amount },
}
