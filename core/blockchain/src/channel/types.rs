use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};

// ============================================================================
// Errors
// ============================================================================

/// Errors related to payment channel operations
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ChannelError {
    #[error("Channel not found: {0:?}")]
    ChannelNotFound(Hash),

    #[error("Channel is not in Open status")]
    ChannelNotOpen,

    #[error("Channel has expired")]
    ChannelExpired,

    #[error("Insufficient deposit for channel operation")]
    InsufficientDeposit,

    #[error("Invalid balances: sender + receiver must equal deposit")]
    InvalidBalances,

    #[error("Invalid nonce: nonce must be strictly increasing")]
    InvalidNonce,

    #[error("Address {0:?} is not a participant in this channel")]
    NotParticipant(Address),

    #[error("Dispute period has not ended yet")]
    DisputePeriodNotOver,

    #[error("Sender and receiver cannot be the same address")]
    SameParticipants,
}

// ============================================================================
// ChannelStatus
// ============================================================================

/// The lifecycle status of a payment channel
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelStatus {
    /// Channel is open and accepting off-chain updates
    Open,
    /// Cooperative or unilateral close is in progress
    Closing { initiated_at: BlockHeight },
    /// A dispute has been filed and is awaiting resolution
    Disputed { disputed_at: BlockHeight },
    /// Channel has been finalised and funds disbursed
    Closed,
    /// Channel passed its expiry block without being closed
    Expired,
}

// ============================================================================
// Channel
// ============================================================================

/// A two-party state channel for high-frequency micropayments
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Channel {
    /// Unique channel identifier (derived from sender, receiver, open height)
    pub id: Hash,
    /// The party that deposits funds (payer / user / agent)
    pub sender: Address,
    /// The party that accumulates funds (payee / provider / agent)
    pub receiver: Address,
    /// Total amount deposited by the sender
    pub deposit: Amount,
    /// Sender's remaining balance inside the channel
    pub sender_balance: Amount,
    /// Accumulated balance owed to the receiver
    pub receiver_balance: Amount,
    /// Monotonically-increasing state-update counter
    pub nonce: u64,
    /// Current lifecycle status
    pub status: ChannelStatus,
    /// Block height at which the channel was opened
    pub opened_at: BlockHeight,
    /// Block height at which the channel auto-expires if not closed
    pub expires_at: BlockHeight,
    /// Block height of the most recent state update
    pub last_updated: BlockHeight,
    /// Number of blocks that must pass after a close/dispute before finalisation
    pub dispute_period: u64,
}

// ============================================================================
// ChannelUpdate
// ============================================================================

/// An off-chain (or on-chain dispute) state update for a channel
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelUpdate {
    /// The channel this update targets
    pub channel_id: Hash,
    /// New nonce — must be strictly greater than the current channel nonce
    pub nonce: u64,
    /// Proposed sender balance after the update
    pub sender_balance: Amount,
    /// Proposed receiver balance after the update
    pub receiver_balance: Amount,
}
