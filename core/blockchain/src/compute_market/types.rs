use crate::types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Compute marketplace state
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComputeMarketState {
    /// Registered providers by address
    pub providers: HashMap<Address, ProviderInfo>,

    /// Active jobs by job ID
    pub jobs: HashMap<Hash, ComputeJob>,

    /// Jobs by user address (for lookup)
    pub user_jobs: HashMap<Address, Vec<Hash>>,

    /// Jobs by provider address (for lookup)
    pub provider_jobs: HashMap<Address, Vec<Hash>>,

    /// Active disputes by dispute ID
    pub disputes: HashMap<Hash, ComputeDispute>,

    /// Protocol treasury balance (from fees)
    pub treasury_balance: Amount,

    /// Total value locked in escrow
    pub total_escrow: Amount,

    /// Total provider stakes
    pub total_provider_stake: Amount,

    /// Marketplace statistics
    pub stats: MarketStats,
}

impl ComputeMarketState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Marketplace statistics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MarketStats {
    pub total_providers: u64,
    pub active_providers: u64,
    pub total_jobs_created: u64,
    pub total_jobs_completed: u64,
    pub total_jobs_failed: u64,
    pub total_jobs_disputed: u64,
    pub total_volume: Amount,
}

/// Settlement result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettlementResult {
    pub job_id: Hash,
    pub provider_payment: Amount,
    pub user_refund: Amount,
    pub protocol_fee: Amount,
}

/// Compute market errors
#[derive(Debug, thiserror::Error)]
pub enum ComputeMarketError {
    #[error("Provider already registered")]
    ProviderAlreadyRegistered,

    #[error("Provider not found")]
    ProviderNotFound,

    #[error("Provider not active")]
    ProviderNotActive,

    #[error("Insufficient stake")]
    InsufficientStake,

    #[error("Invalid status transition")]
    InvalidStatusTransition,

    #[error("Provider has active jobs")]
    HasActiveJobs,

    #[error("Job already exists")]
    JobAlreadyExists,

    #[error("Job not found")]
    JobNotFound,

    #[error("Invalid job state for this operation")]
    InvalidJobState,

    #[error("Resource type not supported by provider")]
    ResourceTypeNotSupported,

    #[error("Insufficient provider capacity")]
    InsufficientCapacity,

    #[error("Not authorized")]
    NotAuthorized,

    #[error("Price exceeds user's maximum")]
    PriceExceedsMax,

    #[error("No provider assigned")]
    NoProvider,

    #[error("Dispute window expired")]
    DisputeWindowExpired,

    #[error("Dispute not found")]
    DisputeNotFound,

    #[error("Dispute already resolved")]
    DisputeAlreadyResolved,
}
