use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};

// ============================================================================
// BatchStatus
// ============================================================================

/// Lifecycle state of a batch settlement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchStatus {
    /// Batch has been created and is awaiting processing.
    Pending,
    /// Batch is currently being processed.
    Processing,
    /// All settlements in the batch completed successfully.
    Completed,
    /// Batch completed with some successes and some failures.
    PartiallyCompleted { succeeded: usize, failed: usize },
    /// Batch failed entirely; reason attached.
    Failed(String),
}

// ============================================================================
// BatchSettlementError
// ============================================================================

/// Errors that can arise during batch settlement operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BatchSettlementError {
    #[error("Cannot create a batch with no settlements")]
    EmptyBatch,

    #[error("Batch size {actual} exceeds maximum allowed {max}")]
    BatchTooLarge { max: usize, actual: usize },

    #[error("Invalid settlement: {0}")]
    InvalidSettlement(String),

    #[error("Channel not found: {0:?}")]
    ChannelNotFound(Hash),

    #[error("Channel already settled: {0:?}")]
    AlreadySettled(Hash),
}

// ============================================================================
// ChannelSettlement
// ============================================================================

/// A single channel's contribution to a batch settlement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelSettlement {
    /// The channel being settled.
    pub channel_id: Hash,
    /// Sender (payer) address.
    pub sender: Address,
    /// Receiver (payee) address.
    pub receiver: Address,
    /// Amount transferred from sender to receiver.
    pub amount: Amount,
    /// Protocol fee deducted from `amount`.
    pub fee: Amount,
    /// Channel nonce at the time of settlement (monotonically increasing).
    pub final_nonce: u64,
}

// ============================================================================
// BatchSettlement
// ============================================================================

/// An aggregated settlement of multiple payment channels written to the chain
/// in a single operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchSettlement {
    /// Unique batch identifier (deterministic hash of all settlements).
    pub id: Hash,
    /// Individual channel settlements included in this batch.
    pub settlements: Vec<ChannelSettlement>,
    /// Sum of all `amount` fields across settlements.
    pub total_amount: Amount,
    /// Sum of all `fee` fields across settlements.
    pub total_fees: Amount,
    /// Block height at which the batch was created.
    pub height: BlockHeight,
    /// Current lifecycle status.
    pub status: BatchStatus,
}
