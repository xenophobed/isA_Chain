use crate::types::*;
use crate::account::AccountError;
use crate::block::BlockError;
use crate::token::TokenError;
use crate::transaction::TransactionError;

/// Main blockchain error type
#[derive(Debug, thiserror::Error)]
pub enum BlockchainError {
    #[error("Block error: {0}")]
    Block(#[from] BlockError),
    
    #[error("Transaction error: {0}")]
    Transaction(#[from] TransactionError),
    
    #[error("Account error: {0}")]
    Account(#[from] AccountError),
    
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    
    #[error("Consensus error: {0}")]
    Consensus(#[from] ConsensusError),
    
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),
    
    #[error("State error: {0}")]
    State(#[from] StateError),
    
    #[error("Mempool error: {0}")]
    Mempool(#[from] MempoolError),
    
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    #[error("Token error: {0}")]
    Token(#[from] TokenError),
}

/// Storage-related errors
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Column family not found: {0}")]
    ColumnFamilyNotFound(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Block not found: {hash}")]
    BlockNotFound { hash: Hash },
    
    #[error("Transaction not found: {hash}")]
    TransactionNotFound { hash: Hash },
    
    #[error("Account not found: {address}")]
    AccountNotFound { address: Address },
    
    #[error("State root not found: {root}")]
    StateRootNotFound { root: Hash },
    
    #[error("Corruption detected in block {height}")]
    DataCorruption { height: BlockHeight },
    
    #[error("Storage full")]
    StorageFull,
    
    #[error("Access denied")]
    AccessDenied,
}

/// Consensus-related errors
#[derive(Debug, thiserror::Error)]
pub enum ConsensusError {
    #[error("Invalid validator set")]
    InvalidValidatorSet,
    
    #[error("Insufficient voting power: required {required}, got {actual}")]
    InsufficientVotingPower { required: Amount, actual: Amount },
    
    #[error("Double sign detected for validator {validator} at height {height}")]
    DoubleSign { validator: Address, height: BlockHeight },
    
    #[error("Invalid block proposal from {proposer}")]
    InvalidProposal { proposer: Address },
    
    #[error("Consensus timeout at height {height}")]
    ConsensusTimeout { height: BlockHeight },
    
    #[error("Invalid consensus round: {round}")]
    InvalidRound { round: u32 },
    
    #[error("Fork detected at height {height}")]
    ForkDetected { height: BlockHeight },
    
    #[error("Finality violation")]
    FinalityViolation,
    
    #[error("Validator {validator} is jailed")]
    ValidatorJailed { validator: Address },
    
    #[error("Validator {validator} is slashed")]
    ValidatorSlashed { validator: Address },
}

/// Network-related errors
#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Peer not found: {peer_id}")]
    PeerNotFound { peer_id: String },
    
    #[error("Invalid message from peer {peer_id}")]
    InvalidMessage { peer_id: String },
    
    #[error("Network timeout")]
    Timeout,
    
    #[error("Protocol version mismatch: expected {expected}, got {actual}")]
    ProtocolMismatch { expected: u32, actual: u32 },
    
    #[error("Sync error: {0}")]
    SyncError(String),
    
    #[error("Gossip protocol error: {0}")]
    GossipError(String),
    
    #[error("Peer blacklisted: {peer_id}")]
    PeerBlacklisted { peer_id: String },
    
    #[error("Network partition detected")]
    NetworkPartition,
    
    #[error("Rate limit exceeded for peer {peer_id}")]
    RateLimitExceeded { peer_id: String },
}

/// State management errors
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("State transition failed: {reason}")]
    TransitionFailed { reason: String },
    
    #[error("Invalid state root: expected {expected}, got {actual}")]
    InvalidStateRoot { expected: Hash, actual: Hash },
    
    #[error("State not found for height {height}")]
    StateNotFound { height: BlockHeight },
    
    #[error("Merkle proof verification failed")]
    MerkleProofFailed,
    
    #[error("State tree corruption")]
    TreeCorruption,
    
    #[error("State rollback failed")]
    RollbackFailed,
    
    #[error("Checkpoint creation failed")]
    CheckpointFailed,
    
    #[error("State pruning error: {0}")]
    PruningError(String),
}

/// Mempool-related errors
#[derive(Debug, thiserror::Error)]
pub enum MempoolError {
    #[error("Transaction already exists: {hash}")]
    TransactionExists { hash: Hash },
    
    #[error("Mempool full")]
    MempoolFull,
    
    #[error("Transaction evicted: {hash}")]
    TransactionEvicted { hash: Hash },
    
    #[error("Nonce gap: expected {expected}, got {actual}")]
    NonceGap { expected: u64, actual: u64 },
    
    #[error("Gas price too low: minimum {minimum}, got {actual}")]
    GasPriceTooLow { minimum: GasPrice, actual: GasPrice },
    
    #[error("Transaction too large: {size} bytes")]
    TransactionTooLarge { size: usize },
    
    #[error("Replacement transaction underpriced")]
    ReplacementUnderpriced,
    
    #[error("Account {address} has too many pending transactions")]
    TooManyPendingTransactions { address: Address },
}

/// Validation errors
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Invalid chain ID: expected {expected}, got {actual}")]
    InvalidChainId { expected: ChainId, actual: ChainId },
    
    #[error("Invalid block height: expected {expected}, got {actual}")]
    InvalidBlockHeight { expected: BlockHeight, actual: BlockHeight },
    
    #[error("Invalid timestamp: {timestamp}")]
    InvalidTimestamp { timestamp: Timestamp },
    
    #[error("Gas limit exceeded: limit {limit}, used {used}")]
    GasLimitExceeded { limit: Gas, used: Gas },
    
    #[error("Invalid transaction order")]
    InvalidTransactionOrder,
    
    #[error("Duplicate transaction: {hash}")]
    DuplicateTransaction { hash: Hash },
    
    #[error("Invalid merkle root")]
    InvalidMerkleRoot,
    
    #[error("Block size exceeded: {size} bytes")]
    BlockSizeExceeded { size: usize },
    
    #[error("Invalid validator signature")]
    InvalidValidatorSignature,
    
    #[error("Future block: height {height}")]
    FutureBlock { height: BlockHeight },

    #[error("Invalid parent hash")]
    InvalidParentHash,

    #[error("Insufficient balance")]
    InsufficientBalance,
}

/// Result type alias for blockchain operations
pub type BlockchainResult<T> = Result<T, BlockchainError>;

/// Convert common errors to blockchain errors
impl From<bincode::Error> for StorageError {
    fn from(err: bincode::Error) -> Self {
        StorageError::Serialization(err.to_string())
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(err: serde_json::Error) -> Self {
        StorageError::Serialization(err.to_string())
    }
}

impl From<rocksdb::Error> for StorageError {
    fn from(err: rocksdb::Error) -> Self {
        StorageError::Database(err.to_string())
    }
}

/// Error code enumeration for API responses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // General errors (1000-1099)
    InternalError = 1000,
    InvalidRequest = 1001,
    NotFound = 1002,
    Unauthorized = 1003,
    RateLimited = 1004,
    
    // Block errors (1100-1199)
    InvalidBlock = 1100,
    BlockNotFound = 1101,
    InvalidBlockHash = 1102,
    InvalidBlockHeight = 1103,
    
    // Transaction errors (1200-1299)
    InvalidTransaction = 1200,
    TransactionNotFound = 1201,
    InsufficientBalance = 1202,
    InvalidNonce = 1203,
    GasLimitExceeded = 1204,
    TransactionTimeout = 1205,
    
    // Account errors (1300-1399)
    AccountNotFound = 1300,
    InsufficientFunds = 1301,
    InvalidAddress = 1302,
    
    // Consensus errors (1400-1499)
    ConsensusFailure = 1400,
    InvalidValidator = 1401,
    DoubleSign = 1402,
    
    // Network errors (1500-1599)
    NetworkError = 1500,
    PeerNotFound = 1501,
    SyncError = 1502,
    
    // Storage errors (1600-1699)
    StorageError = 1600,
    DataCorruption = 1601,
    StorageFull = 1602,
}

impl ErrorCode {
    /// Get error message for the code
    pub fn message(&self) -> &'static str {
        match self {
            ErrorCode::InternalError => "Internal server error",
            ErrorCode::InvalidRequest => "Invalid request",
            ErrorCode::NotFound => "Resource not found",
            ErrorCode::Unauthorized => "Unauthorized access",
            ErrorCode::RateLimited => "Rate limit exceeded",
            
            ErrorCode::InvalidBlock => "Invalid block",
            ErrorCode::BlockNotFound => "Block not found",
            ErrorCode::InvalidBlockHash => "Invalid block hash",
            ErrorCode::InvalidBlockHeight => "Invalid block height",
            
            ErrorCode::InvalidTransaction => "Invalid transaction",
            ErrorCode::TransactionNotFound => "Transaction not found",
            ErrorCode::InsufficientBalance => "Insufficient balance",
            ErrorCode::InvalidNonce => "Invalid nonce",
            ErrorCode::GasLimitExceeded => "Gas limit exceeded",
            ErrorCode::TransactionTimeout => "Transaction timeout",
            
            ErrorCode::AccountNotFound => "Account not found",
            ErrorCode::InsufficientFunds => "Insufficient funds",
            ErrorCode::InvalidAddress => "Invalid address",
            
            ErrorCode::ConsensusFailure => "Consensus failure",
            ErrorCode::InvalidValidator => "Invalid validator",
            ErrorCode::DoubleSign => "Double sign detected",
            
            ErrorCode::NetworkError => "Network error",
            ErrorCode::PeerNotFound => "Peer not found",
            ErrorCode::SyncError => "Synchronization error",
            
            ErrorCode::StorageError => "Storage error",
            ErrorCode::DataCorruption => "Data corruption detected",
            ErrorCode::StorageFull => "Storage full",
        }
    }
}

/// API error response format
#[derive(Debug, serde::Serialize)]
pub struct ApiError {
    pub code: u32,
    pub message: String,
    pub details: Option<String>,
}

impl ApiError {
    pub fn new(code: ErrorCode, details: Option<String>) -> Self {
        ApiError {
            code: code as u32,
            message: code.message().to_string(),
            details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_codes() {
        assert_eq!(ErrorCode::InternalError as u32, 1000);
        assert_eq!(ErrorCode::InvalidBlock as u32, 1100);
        assert_eq!(ErrorCode::InvalidTransaction as u32, 1200);
    }
    
    #[test]
    fn test_api_error() {
        let error = ApiError::new(ErrorCode::InvalidTransaction, Some("Invalid signature".to_string()));
        assert_eq!(error.code, 1200);
        assert_eq!(error.message, "Invalid transaction");
        assert_eq!(error.details, Some("Invalid signature".to_string()));
    }
}