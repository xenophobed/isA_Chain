use super::primitives::*;
use serde::{Deserialize, Serialize};

/// Resource types available in the compute marketplace
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType {
    /// Virtual machine with full OS
    VM,
    /// Browser instance for web automation
    Browser,
    /// Code execution environment (REPL)
    REPL,
    /// Desktop environment with GUI
    Desktop,
    /// Full agent runtime (VM + Browser + Tools)
    AgentRuntime,
}

/// Hardware specifications for compute resources
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeCapacity {
    /// CPU cores (millicores, 1000 = 1 core)
    pub cpu_millicores: u32,
    /// Memory in MB
    pub memory_mb: u32,
    /// Storage in GB
    pub storage_gb: u32,
    /// GPU memory in MB (0 if no GPU)
    pub gpu_memory_mb: u32,
    /// Network bandwidth in Mbps
    pub bandwidth_mbps: u32,
}

impl ComputeCapacity {
    pub fn new(cpu_millicores: u32, memory_mb: u32, storage_gb: u32) -> Self {
        Self {
            cpu_millicores,
            memory_mb,
            storage_gb,
            gpu_memory_mb: 0,
            bandwidth_mbps: 100,
        }
    }

    /// Standard VM: 2 cores, 4GB RAM, 20GB storage
    pub fn standard_vm() -> Self {
        Self::new(2000, 4096, 20)
    }

    /// Standard browser: 1 core, 2GB RAM, 5GB storage
    pub fn standard_browser() -> Self {
        Self::new(1000, 2048, 5)
    }

    /// Standard agent runtime: 4 cores, 8GB RAM, 50GB storage
    pub fn standard_agent() -> Self {
        Self::new(4000, 8192, 50)
    }
}

/// Provider status in the marketplace
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderStatus {
    /// Provider is registered but not yet active
    Pending,
    /// Provider is active and accepting jobs
    Active,
    /// Provider is temporarily paused (by provider)
    Paused,
    /// Provider is suspended (by protocol, e.g., slashing)
    Suspended,
    /// Provider has exited the marketplace
    Exited,
}

/// Compute job status
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    /// Job is pending provider acceptance
    Pending,
    /// Job is matched to a provider, awaiting start
    Matched,
    /// Job is currently running
    Running,
    /// Job completed successfully
    Completed,
    /// Job failed
    Failed,
    /// Job was cancelled by user
    Cancelled,
    /// Job is in dispute resolution
    Disputed,
}

/// Compute provider registration info
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderInfo {
    /// Provider's wallet address
    pub address: Address,
    /// Supported resource types
    pub resource_types: Vec<ResourceType>,
    /// Total capacity offered
    pub capacity: ComputeCapacity,
    /// Available capacity (not currently allocated)
    pub available_capacity: ComputeCapacity,
    /// Price per hour in ISA tokens (wei)
    pub price_per_hour: Amount,
    /// Minimum job duration in seconds
    pub min_duration_secs: u64,
    /// Maximum job duration in seconds
    pub max_duration_secs: u64,
    /// Provider's stake amount
    pub stake: Amount,
    /// Provider status
    pub status: ProviderStatus,
    /// Reputation score (0-10000, basis points)
    pub reputation: u32,
    /// Total jobs completed
    pub jobs_completed: u64,
    /// Total jobs failed
    pub jobs_failed: u64,
    /// Geographic region (optional)
    pub region: Option<String>,
    /// Endpoint URL for pool_manager communication
    pub endpoint: String,
}

/// Compute job definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComputeJob {
    /// Unique job ID
    pub job_id: Hash,
    /// User who requested the job
    pub user: Address,
    /// Assigned provider (None if pending)
    pub provider: Option<Address>,
    /// Required resource type
    pub resource_type: ResourceType,
    /// Required capacity
    pub capacity: ComputeCapacity,
    /// Maximum price willing to pay per hour
    pub max_price_per_hour: Amount,
    /// Requested duration in seconds
    pub duration_secs: u64,
    /// Escrowed payment amount
    pub escrow_amount: Amount,
    /// Job status
    pub status: JobStatus,
    /// Job creation timestamp
    pub created_at: Timestamp,
    /// Job start timestamp (when matched)
    pub started_at: Option<Timestamp>,
    /// Job end timestamp
    pub ended_at: Option<Timestamp>,
    /// Actual usage metrics (for settlement)
    pub actual_usage: Option<ComputeUsage>,
}

/// Actual compute usage for settlement
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeUsage {
    /// Actual duration in seconds
    pub duration_secs: u64,
    /// CPU seconds consumed
    pub cpu_seconds: u64,
    /// Memory MB-seconds consumed
    pub memory_mb_seconds: u64,
    /// Network bytes transferred
    pub network_bytes: u64,
    /// Storage bytes used
    pub storage_bytes: u64,
}

/// Settlement proof for job completion
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettlementProof {
    /// Job being settled
    pub job_id: Hash,
    /// Final usage metrics
    pub usage: ComputeUsage,
    /// Provider's signature on usage
    pub provider_signature: Signature,
    /// User's signature on usage (optional, for disputes)
    pub user_signature: Option<Signature>,
    /// Merkle proof of execution (optional, for verification)
    pub execution_proof: Option<Vec<u8>>,
}

/// Dispute types for compute jobs
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisputeType {
    /// Provider didn't deliver promised resources
    NonDelivery,
    /// Resources were below specified capacity
    UnderCapacity,
    /// Job failed due to provider issues
    ProviderFault,
    /// User didn't pay (shouldn't happen with escrow)
    NonPayment,
    /// Usage metrics disagreement
    UsageDispute,
}

/// Dispute record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComputeDispute {
    /// Dispute ID
    pub dispute_id: Hash,
    /// Job in dispute
    pub job_id: Hash,
    /// Who initiated the dispute
    pub initiator: Address,
    /// Dispute type
    pub dispute_type: DisputeType,
    /// Evidence hash (off-chain evidence)
    pub evidence_hash: Hash,
    /// Dispute creation time
    pub created_at: Timestamp,
    /// Resolution deadline
    pub deadline: Timestamp,
    /// Resolution (None if pending)
    pub resolution: Option<DisputeResolution>,
}

/// Dispute resolution outcome
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DisputeResolution {
    /// Who won the dispute
    pub winner: Address,
    /// Amount refunded to user
    pub user_refund: Amount,
    /// Amount paid to provider
    pub provider_payment: Amount,
    /// Slash amount from provider stake
    pub slash_amount: Amount,
    /// Resolver (governance or arbitrator)
    pub resolved_by: Address,
    /// Resolution timestamp
    pub resolved_at: Timestamp,
}
