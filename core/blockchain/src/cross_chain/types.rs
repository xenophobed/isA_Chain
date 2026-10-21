use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};

// ============================================================================
// ChainType
// ============================================================================

/// Identifies a blockchain network participating in the bridge.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChainType {
    /// The native isA_Chain
    ISAChain,
    /// Ethereum mainnet (chain ID 1)
    Ethereum,
    /// Base L2 (chain ID 8453)
    Base,
    /// Any other chain identified by name
    Custom(String),
}

// ============================================================================
// TransferStatus
// ============================================================================

/// Lifecycle state of a cross-chain transfer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferStatus {
    /// Transfer submitted; not yet locked on source chain.
    Pending,
    /// ISA tokens locked in bridge escrow on source chain.
    Locked,
    /// Bridge relayer has submitted the mint on the destination chain.
    Minting,
    /// Mint confirmed on destination chain; transfer complete.
    Completed,
    /// Transfer failed; reason is included.
    Failed(String),
    /// Transfer was refunded back to the original sender.
    Refunded,
}

// ============================================================================
// CrossChainTransfer
// ============================================================================

/// An individual cross-chain transfer record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossChainTransfer {
    /// Unique transfer identifier (Blake3 hash of sender + amount + height).
    pub id: Hash,
    /// Source blockchain.
    pub source_chain: ChainType,
    /// Destination blockchain.
    pub dest_chain: ChainType,
    /// Sender's ISA chain address.
    pub sender: Address,
    /// Destination address on the external chain (0x-prefixed hex string).
    pub recipient_external: String,
    /// Gross amount of ISA tokens being transferred.
    pub amount: Amount,
    /// Bridge fee deducted from `amount` before minting on destination.
    pub fee: Amount,
    /// Current transfer lifecycle status.
    pub status: TransferStatus,
    /// Block height on isA_Chain when the transfer was created.
    pub created_at: BlockHeight,
    /// Block height when the transfer reached a terminal state.
    pub completed_at: Option<BlockHeight>,
    /// Transaction hash on the destination/external chain (set by relayer).
    pub external_tx_hash: Option<String>,
}

// ============================================================================
// BridgeError
// ============================================================================

/// Errors that can occur during bridge operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BridgeError {
    #[error("Chain type is not supported by this bridge")]
    UnsupportedChain,

    #[error("Transfer amount is below the minimum required")]
    InsufficientAmount,

    #[error("Transfer amount exceeds the bridge maximum")]
    ExceedsMaxTransfer,

    #[error("Transfer not found: {0:?}")]
    TransferNotFound(Hash),

    #[error("Address is not an authorized relayer: {0:?}")]
    UnauthorizedRelayer(Address),

    #[error("Transfer has already reached a terminal state")]
    AlreadyCompleted,

    #[error("Recipient address is invalid (must be non-empty 0x-prefixed hex)")]
    InvalidAddress,

    #[error("Only the admin may perform this operation")]
    UnauthorizedAdmin,
}
