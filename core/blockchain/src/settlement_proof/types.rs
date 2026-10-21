use crate::settlement::ServiceType;
use crate::types::{Address, Amount, BlockHeight, Hash, Timestamp};
use serde::{Deserialize, Serialize};

// ============================================================================
// ProofStatus
// ============================================================================

/// Lifecycle state of a settlement proof.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofStatus {
    /// Transaction submitted to chain, awaiting confirmations.
    Pending,
    /// Confirmed with enough block confirmations (>= confirmations_required).
    Confirmed,
    /// Deeply confirmed and considered immutable (>= finalization_depth).
    Finalized,
    /// Transaction failed; reason attached.
    Failed(String),
}

// ============================================================================
// SettlementProofRecord
// ============================================================================

/// On-chain proof that links an off-chain billing event to an on-chain
/// transaction hash, anchoring the billing record permanently.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettlementProofRecord {
    /// Unique proof ID (deterministic hash of inputs).
    pub proof_id: Hash,
    /// References the `SettlementRecord` this proof covers.
    pub settlement_id: Hash,
    /// On-chain transaction hash that executed the settlement.
    pub tx_hash: Hash,
    /// Off-chain billing event ID (e.g. from isA billing service).
    pub billing_event_id: String,
    /// Address that paid (ISA burned from this address).
    pub user: Address,
    /// Address that received payment (ISA minted to this address).
    pub provider: Address,
    /// Gross billed amount.
    pub amount: Amount,
    /// Protocol fee portion of the amount.
    pub fee: Amount,
    /// Which isA service was billed.
    pub service_type: ServiceType,
    /// Block height at which the transaction was included.
    pub block_height: BlockHeight,
    /// Unix timestamp (milliseconds) of proof creation.
    pub timestamp: Timestamp,
    /// Number of confirmations the transaction has accumulated.
    pub confirmations: u64,
    /// Current proof lifecycle status.
    pub status: ProofStatus,
}

// ============================================================================
// SettlementProofError
// ============================================================================

/// Errors that can arise during proof recording or verification.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SettlementProofError {
    #[error("Proof not found: {0:?}")]
    ProofNotFound(Hash),

    #[error("Duplicate proof for settlement: {0:?}")]
    DuplicateProof(Hash),

    #[error("Billing event already recorded: {0}")]
    DuplicateBillingEvent(String),

    #[error("Proof is invalid")]
    InvalidProof,

    #[error("Settlement not found: {0:?}")]
    SettlementNotFound(Hash),
}
