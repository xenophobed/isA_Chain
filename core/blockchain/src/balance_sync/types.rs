use crate::types::{Address, Amount, BlockHeight};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Default auto-fix threshold: 1_000_000 (small discrepancies only)
pub const DEFAULT_AUTO_FIX_THRESHOLD: Amount = 1_000_000;

/// Default sync interval: every 100 blocks
pub const DEFAULT_SYNC_INTERVAL_BLOCKS: u64 = 100;

/// Errors that can occur during balance synchronisation
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SyncError {
    #[error("Account not found: {0}")]
    AccountNotFound(Address),

    #[error("Sync already in progress")]
    SyncInProgress,

    #[error("Invalid state for sync operation")]
    InvalidState,
}

/// A snapshot of one account's balance state at a given block height,
/// together with the computed discrepancy between on-chain ISA value and
/// off-chain credits.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalanceRecord {
    /// Account address
    pub address: Address,
    /// On-chain ISA token balance (in ISA wei)
    pub on_chain_isa: Amount,
    /// Current off-chain credit balance (in micro-credits)
    pub off_chain_credits: Amount,
    /// What the off-chain credit balance *should* be based on the ISA value
    /// at the current oracle prices
    pub expected_credits: Amount,
    /// `off_chain_credits as i128 - expected_credits as i128`
    /// Positive → account was over-credited; negative → under-credited
    pub discrepancy: i128,
    /// Block height at which this record was produced
    pub last_synced: BlockHeight,
}

/// Aggregate result of one full reconciliation pass
#[derive(Clone, Debug)]
pub struct SyncResult {
    /// Total number of accounts examined
    pub total_accounts: usize,
    /// Number of accounts successfully processed (no error)
    pub synced: usize,
    /// Number of accounts where a discrepancy was detected
    pub discrepancies_found: usize,
    /// Sum of all `discrepancy` values across the pass (can be negative)
    pub total_discrepancy: i128,
    /// Individual records for every account examined
    pub records: Vec<BalanceRecord>,
    /// Block height at which the sync ran
    pub height: BlockHeight,
}

/// The recommended action after examining a single `BalanceRecord`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncAction {
    /// Balances match — nothing to do
    NoAction,
    /// Apply a credit adjustment; positive = add credits, negative = deduct
    CreditAdjustment(i128),
    /// Discrepancy is too large for automatic resolution — needs human review
    FlagForReview,
    /// Account has pending settlements; skip this cycle
    PendingSettlement,
}
