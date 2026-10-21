use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};

// ============================================================================
// Constants
// ============================================================================

/// Default number of blocks before a deferred settlement expires.
pub const DEFAULT_MAX_DEFER_BLOCKS: u64 = 1_000;

/// Default shortfall threshold below which credits are auto-deducted.
pub const DEFAULT_AUTO_CREDIT_THRESHOLD: Amount = 1_000_000;

// ============================================================================
// FallbackAction
// ============================================================================

/// The action taken when a user has insufficient on-chain ISA balance at
/// settlement time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FallbackAction {
    /// Deduct the shortfall from the user's off-chain credit balance instead.
    CreditDeduction,

    /// Settle the available portion now; record the remainder as outstanding.
    PartialSettlement {
        /// Amount that was successfully settled.
        settled: Amount,
        /// Amount still owed (the shortfall).
        remaining: Amount,
    },

    /// Defer the full settlement until `deadline` block height.
    DeferredSettlement {
        /// Block height by which the settlement must be resolved.
        deadline: BlockHeight,
    },

    /// The provider extends credit to the user (trusted relationship).
    ProviderCredit,

    /// Settlement rejected entirely — no fallback available.
    Rejected,
}

// ============================================================================
// FallbackRecord
// ============================================================================

/// Immutable record of a single fallback event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FallbackRecord {
    /// Unique ID for this fallback record.
    pub id: Hash,
    /// ID of the original settlement that triggered the fallback.
    pub original_settlement_id: Hash,
    /// The user who could not cover the settlement.
    pub user: Address,
    /// The provider who was owed the settlement amount.
    pub provider: Address,
    /// The full amount the settlement required.
    pub required_amount: Amount,
    /// The user's on-chain ISA balance at the time of the shortfall.
    pub available_balance: Amount,
    /// `required_amount - available_balance`.
    pub shortfall: Amount,
    /// Which fallback strategy was applied.
    pub action_taken: FallbackAction,
    /// Block height at which the fallback was recorded.
    pub height: BlockHeight,
    /// Whether this record has been fully resolved.
    pub resolved: bool,
    /// Block height at which this record was resolved, if applicable.
    pub resolved_at: Option<BlockHeight>,
}

// ============================================================================
// FallbackError
// ============================================================================

/// Errors that can arise during fallback handling.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FallbackError {
    #[error("No fallback available for the given shortfall")]
    NoFallbackAvailable,

    #[error("Fallback record already resolved: {0:?}")]
    AlreadyResolved(Hash),

    #[error("Fallback record not found: {0:?}")]
    RecordNotFound(Hash),

    #[error("Deferred settlement deadline has expired")]
    DeadlineExpired,

    #[error("Insufficient credits to cover the shortfall")]
    InsufficientCredits,
}
