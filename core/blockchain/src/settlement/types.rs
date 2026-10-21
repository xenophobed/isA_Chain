use crate::types::{Address, Amount, BlockHeight, Hash, Timestamp};
use serde::{Deserialize, Serialize};

/// Which isA service was billed for this settlement.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServiceType {
    /// isA_Model — model inference
    ModelInference,
    /// isA_MCP — tool execution
    ToolExecution,
    /// isA_OS — compute usage
    ComputeUsage,
    /// isA_Data — storage
    Storage,
    /// isA_Agent — agent runtime
    AgentRuntime,
    /// Extensible catch-all
    Custom(String),
}

/// Lifecycle state of a settlement record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettlementStatus {
    Pending,
    Completed,
    /// Settlement failed; reason attached.
    Failed(String),
    Disputed,
}

/// Immutable record of a single burn-mint settlement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettlementRecord {
    /// Unique settlement ID (hash of inputs).
    pub id: Hash,
    /// Address that paid (ISA burned from this address).
    pub user: Address,
    /// Address that received payment (ISA minted to this address).
    pub provider: Address,
    /// Total billed amount before fee deduction.
    pub gross_amount: Amount,
    /// Protocol fee deducted from gross.
    pub fee_amount: Amount,
    /// Net amount minted to provider (gross - fee).
    pub net_amount: Amount,
    /// Hash reference for the burn transaction.
    pub burn_tx: Hash,
    /// Hash reference for the mint transaction.
    pub mint_tx: Hash,
    /// Block height at which settlement was processed.
    pub height: BlockHeight,
    /// Unix timestamp (milliseconds) of settlement.
    pub timestamp: Timestamp,
    /// Which service was billed.
    pub service_type: ServiceType,
    /// Current lifecycle status.
    pub status: SettlementStatus,
}

/// Errors that can arise during settlement processing.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SettlementError {
    #[error("Insufficient balance to settle")]
    InsufficientBalance,

    #[error("Invalid amount: must be greater than zero")]
    InvalidAmount,

    #[error("User and provider addresses must differ")]
    SameUserAndProvider,

    #[error("Settlement not found: {0:?}")]
    SettlementNotFound(Hash),

    #[error("Settlement already finalized: {0:?}")]
    AlreadySettled(Hash),

    #[error("Token error: {0}")]
    TokenError(String),
}
