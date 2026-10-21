use crate::types::*;
use serde::{Deserialize, Serialize};
use super::data::TransactionData;

/// Transaction types supported by the blockchain
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TransactionType {
    /// Simple token transfer
    Transfer,
    /// Smart contract deployment
    ContractDeploy,
    /// Smart contract function call
    ContractCall,
    /// Validator staking
    Stake,
    /// Unstaking tokens
    Unstake,
    /// Delegate to validator
    Delegate,
    /// Undelegate from validator
    Undelegate,
    /// Governance proposal submission
    GovernanceProposal,
    /// Governance vote
    GovernanceVote,
    /// Cross-chain bridge transaction
    Bridge,

    // ========== Agent-native Economy ==========
    /// Purchase credits by burning ISA
    CreditPurchase,
    /// Spend credits on a service
    CreditSpend,
    /// Agent wallet autonomous spend
    AgentWalletSpend,
    /// Settlement finalization
    Settlement,

    // ========== Compute Marketplace ==========
    /// Register as a compute provider
    ComputeRegister,
    /// Update provider info (capacity, pricing)
    ComputeUpdateProvider,
    /// Provider exit (begin unstaking)
    ComputeProviderExit,
    /// Request compute resources
    ComputeRequest,
    /// Provider accepts a job
    ComputeAccept,
    /// Mark job as started
    ComputeStart,
    /// Settle a completed job
    ComputeSettle,
    /// Cancel a pending job
    ComputeCancel,
    /// Open a dispute
    ComputeDispute,
    /// Resolve a dispute
    ComputeResolveDispute,
}

/// Complete transaction structure
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transaction {
    /// Transaction sender
    pub from: Address,
    
    /// Transaction nonce (prevents replay attacks)
    pub nonce: u64,
    
    /// Transaction data/payload
    pub data: TransactionData,
    
    /// Maximum gas to use for this transaction
    pub gas_limit: Gas,
    
    /// Gas price (amount to pay per gas unit)
    pub gas_price: GasPrice,
    
    /// Chain ID (prevents cross-chain replay attacks)
    pub chain_id: ChainId,
    
    /// Transaction signature
    pub signature: Option<Signature>,
    
    /// Cached transaction hash
    pub(crate) hash: Option<Hash>,
}

/// Transaction validation errors
#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error("Invalid private key")]
    InvalidPrivateKey,

    #[error("Signing failed")]
    SigningFailed,

    #[error("Missing signature")]
    MissingSignature,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Invalid sender: expected {expected}, recovered {recovered}")]
    InvalidSender { expected: Address, recovered: Address },

    #[error("Invalid amount")]
    InvalidAmount,

    #[error("Empty bytecode")]
    EmptyBytecode,

    #[error("Bytecode too large")]
    BytecodeTooLarge,

    #[error("Insufficient stake amount")]
    InsufficientStakeAmount,

    #[error("Insufficient delegation amount")]
    InsufficientDelegationAmount,

    #[error("Invalid commission rate")]
    InvalidCommissionRate,

    #[error("Invalid nonce: expected {expected}, got {actual}")]
    InvalidNonce { expected: u64, actual: u64 },

    #[error("Gas limit too low")]
    GasLimitTooLow,

    #[error("Gas price too low")]
    GasPriceTooLow,

    #[error("Insufficient balance")]
    InsufficientBalance,

    // ========== Compute Marketplace Errors ==========
    #[error("Insufficient provider stake (minimum: {0} ISA)", constants::PROVIDER_MIN_STAKE)]
    InsufficientProviderStake,

    #[error("No resource types specified")]
    NoResourceTypes,

    #[error("Invalid capacity: CPU and memory must be > 0")]
    InvalidCapacity,

    #[error("Invalid price: must be > 0")]
    InvalidPrice,

    #[error("Invalid endpoint URL")]
    InvalidEndpoint,

    #[error("Invalid duration: must be > 0")]
    InvalidDuration,

    #[error("Insufficient escrow amount")]
    InsufficientEscrow,

    #[error("Job not found")]
    JobNotFound,

    #[error("Provider not found")]
    ProviderNotFound,

    #[error("Provider not active")]
    ProviderNotActive,

    #[error("Job already accepted")]
    JobAlreadyAccepted,

    #[error("Job not in correct state for this operation")]
    InvalidJobState,

    #[error("Not authorized to perform this action")]
    NotAuthorized,

    #[error("Dispute window expired")]
    DisputeWindowExpired,

    #[error("Invalid dispute resolution")]
    InvalidDisputeResolution,

    #[error("Provider capacity exceeded")]
    CapacityExceeded,
}
