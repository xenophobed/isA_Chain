use crate::types::{Address, Amount, BlockHeight};
use serde::{Deserialize, Serialize};

// ============================================================================
// SubnetId
// ============================================================================

/// The 6 isA service subnets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubnetId {
    /// isA_Model — inference workloads
    Model,
    /// isA_MCP — tool execution
    Tools,
    /// isA_OS — compute / VMs
    Compute,
    /// isA_Data — storage
    Storage,
    /// isA_Agent — agent runtime
    Agent,
    /// isA_Trade — marketplace
    Market,
}

// ============================================================================
// SubnetStatus / ProviderSubnetStatus
// ============================================================================

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubnetStatus {
    Active,
    Paused,
    Deprecated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderSubnetStatus {
    Active,
    Inactive,
    Slashed,
}

// ============================================================================
// SubnetConfig
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubnetConfig {
    pub id: SubnetId,
    pub name: String,
    pub description: String,
    /// Minimum stake a provider must post to join this subnet (in ISA wei).
    pub min_provider_stake: Amount,
    /// Fraction of block rewards flowing to this subnet, in basis points.
    /// All active subnets must sum to 10 000.
    pub emission_weight: u32,
    /// Subnet-specific protocol fee, in basis points.
    pub fee_rate_bps: u32,
    /// Hard cap on registered providers.
    pub max_providers: usize,
    pub status: SubnetStatus,
    pub created_at: BlockHeight,
}

// ============================================================================
// SubnetProvider
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubnetProvider {
    pub address: Address,
    pub subnet_id: SubnetId,
    pub stake: Amount,
    /// Quality score in basis points (0 – 10 000).
    pub quality_score: u32,
    pub jobs_completed: u64,
    pub revenue_earned: Amount,
    pub registered_at: BlockHeight,
    pub status: ProviderSubnetStatus,
}

// ============================================================================
// SubnetError
// ============================================================================

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SubnetError {
    #[error("Subnet not found")]
    SubnetNotFound,

    #[error("Subnet already exists")]
    SubnetAlreadyExists,

    #[error("Provider not found")]
    ProviderNotFound,

    #[error("Provider already registered in subnet")]
    ProviderAlreadyRegistered,

    #[error("Insufficient stake: required {required}, provided {provided}")]
    InsufficientStake { required: Amount, provided: Amount },

    #[error("Subnet is at maximum provider capacity")]
    SubnetFull,

    #[error("Subnet is not active")]
    SubnetNotActive,

    #[error("Unauthorized admin: {0}")]
    UnauthorizedAdmin(Address),

    #[error("Invalid emission weights: must sum to 10000")]
    InvalidEmissionWeights,
}

// ============================================================================
// Default stake constants (ISA wei — 1 ISA = 10^18 wei)
// ============================================================================

pub const ISA: Amount = 1_000_000_000_000_000_000; // 1 ISA in wei

pub fn isa(n: u128) -> Amount {
    n * ISA
}
