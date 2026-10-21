use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};

// ====================================================================
// Constants
// ====================================================================

/// Default credit price: $0.00001 USD = 100 micro-USD
pub const DEPOSIT_DEFAULT_CREDIT_PRICE_USD: Amount = 100;


// ====================================================================
// DepositStatus
// ====================================================================

/// Lifecycle status of a deposit request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DepositStatus {
    /// Deposit request created; on-chain burn not yet confirmed.
    Pending,
    /// On-chain burn detected; off-chain credit issuance in flight.
    Processing,
    /// Credits have been issued to the user.
    Completed,
    /// Deposit failed (reason attached).
    Failed(String),
    /// ISA was refunded to the user after a failed deposit.
    Refunded,
}

// ====================================================================
// DepositError
// ====================================================================

/// Errors that can occur during deposit operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DepositError {
    #[error("Insufficient balance to complete the deposit")]
    InsufficientBalance,

    #[error("Invalid amount: must be greater than zero")]
    InvalidAmount,

    #[error("Deposit not found: {0:?}")]
    DepositNotFound(Hash),

    #[error("Deposit has already been processed")]
    AlreadyProcessed,

    #[error("ISA price is stale or zero; cannot calculate credits")]
    PriceStale,
}

// ====================================================================
// DepositRequest
// ====================================================================

/// A record of a single on-chain → off-chain deposit.
///
/// Flow: `Pending` → `Processing` → `Completed`
///                              ↘ `Failed` → `Refunded`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepositRequest {
    /// Unique deposit ID (hash of user + isa_amount + created_at).
    pub id: Hash,
    /// User initiating the deposit.
    pub user: Address,
    /// ISA being deposited (micro-ISA units).
    pub isa_amount: Amount,
    /// Off-chain credits to be issued.
    pub credits_to_receive: Amount,
    /// ISA/USD price at time of deposit (micro-USD per ISA).
    pub isa_price_usd: Amount,
    /// Current lifecycle status.
    pub status: DepositStatus,
    /// Block height when deposit was initiated.
    pub created_at: BlockHeight,
    /// Block height when deposit reached a terminal state.
    pub completed_at: Option<BlockHeight>,
    /// On-chain burn transaction hash (set when Processing or later).
    pub tx_hash: Option<Hash>,
}
