use crate::settlement::ServiceType;
use crate::types::{Address, Amount, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// BillingEvent
// ============================================================================

/// A single billing event received from a NATS subscriber, representing one
/// unit of work (inference call, tool execution, etc.) that will be settled
/// on-chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillingEvent {
    /// Unique billing event ID (used for deduplication).
    pub event_id: String,
    /// Address of the user being charged.
    pub user: Address,
    /// Address of the service provider being paid.
    pub provider: Address,
    /// Amount to settle (gross, before protocol fee).
    pub amount: Amount,
    /// Which isA service generated this billing event.
    pub service_type: ServiceType,
    /// Unix timestamp (milliseconds) when the event was generated.
    pub timestamp: Timestamp,
    /// Provider wallet address for receiving the net payment.
    pub provider_wallet: Address,
    /// Service-specific metadata (e.g. model name, tool ID, job ID).
    pub metadata: HashMap<String, String>,
}

// ============================================================================
// BridgeConfig
// ============================================================================

/// Configuration for the `SettlementBridge`.
#[derive(Clone, Debug)]
pub struct BridgeConfig {
    /// Settle pending events every N blocks (default 20 ≈ 60 s at 3 s blocks).
    pub batch_interval_blocks: u64,
    /// Maximum number of billing events per settlement batch (default 100).
    pub max_batch_size: usize,
    /// Number of recent event IDs to remember for deduplication (default 10 000).
    pub dedup_window: usize,
    /// Protocol fee in basis points deducted during settlement (default 250 = 2.5 %).
    pub fee_rate_bps: u32,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        BridgeConfig {
            batch_interval_blocks: 20,
            max_batch_size: 100,
            dedup_window: 10_000,
            fee_rate_bps: 250,
        }
    }
}

// ============================================================================
// BridgeError
// ============================================================================

/// Errors that can arise while operating the `SettlementBridge`.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BridgeError {
    #[error("Duplicate billing event: {0}")]
    DuplicateEvent(String),

    #[error("Invalid billing event: {0}")]
    InvalidEvent(String),

    #[error("Batch settlement failed: {0}")]
    BatchFailed(String),

    #[error("Event queue is full")]
    QueueFull,

    #[error("Amount must be greater than zero")]
    ZeroAmount,
}

// ============================================================================
// BridgeStats
// ============================================================================

/// Summary statistics for the `SettlementBridge`.
#[derive(Clone, Debug)]
pub struct BridgeStats {
    /// Total billing events successfully ingested and processed.
    pub events_processed: u64,
    /// Total settlement batches created.
    pub batches_created: u64,
    /// Cumulative gross amount sent to the settlement engine.
    pub total_settled: Amount,
    /// Number of events currently waiting in the queue.
    pub queue_size: usize,
    /// Number of events that failed settlement and are held for retry.
    pub failed_count: usize,
}
