use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};

// ====================================================================
// BridgeEventType
// ====================================================================

/// The class of operation being bridged between off-chain credits and on-chain ISA.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeEventType {
    /// Buy credits with ISA — ISA burned on-chain, credits issued off-chain.
    CreditPurchase,
    /// Convert credits to ISA — credits burned off-chain, ISA minted on-chain.
    CreditRedemption,
    /// Move credits between accounts (purely off-chain).
    CreditTransfer,
    /// Admin-granted credits (no ISA movement).
    CreditGrant,
    /// Credits spent on a platform service.
    CreditSpend,
}

// ====================================================================
// BridgeDirection
// ====================================================================

/// Which direction value is flowing across the bridge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeDirection {
    /// ISA is burned on-chain; credits are issued off-chain.
    OnChainToOffChain,
    /// Credits are burned off-chain; ISA is minted on-chain.
    OffChainToOnChain,
}

// ====================================================================
// BridgeEventStatus
// ====================================================================

/// Lifecycle state of a single bridge event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeEventStatus {
    /// Bridge event created; awaiting confirmation from both sides.
    Initiated,
    /// On-chain or off-chain side has confirmed; waiting for the other.
    Confirmed,
    /// Both sides confirmed; bridge event is settled.
    Completed,
    /// Bridge event failed with an attached reason.
    Failed(String),
    /// Bridge event was rolled back after a failure.
    RolledBack,
}

// ====================================================================
// CreditBridgeEvent
// ====================================================================

/// A single bridging event between the off-chain credit_service and on-chain ISA.
///
/// ## Lifecycle
/// `Initiated` → `Confirmed` → `Completed`
///                          ↘ `Failed(reason)` → `RolledBack`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreditBridgeEvent {
    /// Unique event identifier.
    pub id: Hash,
    /// The kind of bridge operation.
    pub event_type: BridgeEventType,
    /// The user account involved.
    pub user: Address,
    /// Off-chain credit units involved in this event.
    pub credit_amount: Amount,
    /// On-chain ISA units (micro-ISA) involved in this event.
    pub isa_amount: Amount,
    /// Direction of value flow.
    pub direction: BridgeDirection,
    /// Block height when the event was initiated.
    pub height: BlockHeight,
    /// Current lifecycle status.
    pub status: BridgeEventStatus,
}

// ====================================================================
// CreditBridgeError
// ====================================================================

/// Errors that can occur during credit bridge operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CreditBridgeError {
    #[error("Event not found")]
    EventNotFound,

    #[error("Invalid amount: must be greater than zero")]
    InvalidAmount,

    #[error("Insufficient credits to complete the bridge operation")]
    InsufficientCredits,

    #[error("Insufficient ISA to complete the bridge operation")]
    InsufficientISA,

    #[error("Bridge event has already been completed")]
    AlreadyCompleted,
}
