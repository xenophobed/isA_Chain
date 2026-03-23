use serde::{Deserialize, Serialize};
use std::fmt;

/// Chain ID type
pub type ChainId = u64;

/// Block height type
pub type BlockHeight = u64;

/// Gas amount type
pub type Gas = u64;

/// Gas price type
pub type GasPrice = u64;

/// Amount/Balance type using u128 for large numbers
pub type Amount = u128;

/// Timestamp type (Unix timestamp in milliseconds)
pub type Timestamp = u64;

/// Hash type - 32 bytes using Blake3
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hash([u8; 32]);

impl Hash {
    pub const ZERO: Self = Hash([0u8; 32]);
    
    pub fn new(data: [u8; 32]) -> Self {
        Hash(data)
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != 32 {
            return Err("Hash must be 32 bytes");
        }
        let mut array = [0u8; 32];
        array.copy_from_slice(bytes);
        Ok(Hash(array))
    }
    
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
    
    /// Generate hash from data using Blake3
    pub fn hash_data(data: &[u8]) -> Self {
        let hash = blake3::hash(data);
        Hash(*hash.as_bytes())
    }
    
    /// Generate Merkle root from list of hashes
    pub fn merkle_root(hashes: &[Hash]) -> Self {
        if hashes.is_empty() {
            return Hash::ZERO;
        }
        
        if hashes.len() == 1 {
            return hashes[0];
        }
        
        let mut current_level = hashes.to_vec();
        
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            
            for chunk in current_level.chunks(2) {
                let combined = if chunk.len() == 2 {
                    let mut combined = Vec::new();
                    combined.extend_from_slice(chunk[0].as_bytes());
                    combined.extend_from_slice(chunk[1].as_bytes());
                    combined
                } else {
                    let mut combined = Vec::new();
                    combined.extend_from_slice(chunk[0].as_bytes());
                    combined.extend_from_slice(chunk[0].as_bytes());
                    combined
                };
                next_level.push(Hash::hash_data(&combined));
            }
            
            current_level = next_level;
        }
        
        current_level[0]
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", hex::encode(&self.0[..8]))
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode(&self.0))
    }
}

impl From<[u8; 32]> for Hash {
    fn from(bytes: [u8; 32]) -> Self {
        Hash(bytes)
    }
}

impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Address type - 20 bytes derived from public key
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Address([u8; 20]);

impl Address {
    pub const ZERO: Self = Address([0u8; 20]);
    
    pub fn new(data: [u8; 20]) -> Self {
        Address(data)
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != 20 {
            return Err("Address must be 20 bytes");
        }
        let mut array = [0u8; 20];
        array.copy_from_slice(bytes);
        Ok(Address(array))
    }
    
    pub fn from_public_key(public_key: &[u8]) -> Self {
        let hash = blake3::hash(public_key);
        let mut address = [0u8; 20];
        address.copy_from_slice(&hash.as_bytes()[12..32]);
        Address(address)
    }
    
    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }
    
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Address({})", hex::encode(&self.0[..4]))
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode(&self.0))
    }
}

impl From<[u8; 20]> for Address {
    fn from(bytes: [u8; 20]) -> Self {
        Address(bytes)
    }
}

impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Signature type
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    pub r: [u8; 32],
    pub s: [u8; 32],
    pub v: u8,
}

impl Signature {
    pub fn new(r: [u8; 32], s: [u8; 32], v: u8) -> Self {
        Signature { r, s, v }
    }
    
    pub fn to_bytes(&self) -> [u8; 65] {
        let mut bytes = [0u8; 65];
        bytes[0..32].copy_from_slice(&self.r);
        bytes[32..64].copy_from_slice(&self.s);
        bytes[64] = self.v;
        bytes
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != 65 {
            return Err("Signature must be 65 bytes");
        }
        
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&bytes[0..32]);
        s.copy_from_slice(&bytes[32..64]);
        let v = bytes[64];
        
        Ok(Signature { r, s, v })
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Signature(r: {}, s: {}, v: {})", 
               hex::encode(&self.r[..4]),
               hex::encode(&self.s[..4]),
               self.v)
    }
}

// ============================================================================
// Compute Market Types
// ============================================================================

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

/// Network-specific constants
pub mod constants {
    use super::*;

    pub const MAIN_CHAIN_ID: ChainId = 15489;
    pub const TEST_CHAIN_ID: ChainId = 15490;

    pub const BLOCK_TIME_MS: u64 = 3000; // 3 seconds
    pub const MAX_BLOCK_SIZE: usize = 1024 * 1024; // 1MB
    pub const MAX_GAS_PER_BLOCK: Gas = 30_000_000;
    pub const BASE_GAS_PRICE: GasPrice = 1_000_000_000; // 1 Gwei

    pub const GENESIS_TIMESTAMP: Timestamp = 1704067200000; // Jan 1, 2024 00:00:00 UTC
    pub const INITIAL_SUPPLY: Amount = 1_000_000_000_000_000_000_000_000_000; // 1B ISA tokens

    pub const VALIDATOR_MIN_STAKE: Amount = 32_000_000_000_000_000_000_000; // 32,000 ISA
    pub const DELEGATION_MIN_AMOUNT: Amount = 1_000_000_000_000_000_000; // 1 ISA

    // Compute marketplace constants
    pub const PROVIDER_MIN_STAKE: Amount = 1_000_000_000_000_000_000_000; // 1,000 ISA
    pub const JOB_MIN_ESCROW: Amount = 100_000_000_000_000_000; // 0.1 ISA
    pub const DISPUTE_WINDOW_SECS: u64 = 86400; // 24 hours
    pub const SETTLEMENT_DELAY_SECS: u64 = 3600; // 1 hour grace period
    pub const MAX_SLASH_PERCENT: u32 = 5000; // 50% max slash
    pub const PROTOCOL_FEE_PERCENT: u32 = 250; // 2.5% protocol fee
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hash_creation() {
        let data = b"hello world";
        let hash = Hash::hash_data(data);
        
        // Should be deterministic
        let hash2 = Hash::hash_data(data);
        assert_eq!(hash, hash2);
        
        // Different data should produce different hash
        let hash3 = Hash::hash_data(b"hello world!");
        assert_ne!(hash, hash3);
    }
    
    #[test]
    fn test_address_from_public_key() {
        let public_key = [1u8; 32];
        let address = Address::from_public_key(&public_key);
        
        // Should be deterministic
        let address2 = Address::from_public_key(&public_key);
        assert_eq!(address, address2);
    }
    
    #[test]
    fn test_merkle_root() {
        let hashes = vec![
            Hash::hash_data(b"tx1"),
            Hash::hash_data(b"tx2"),
            Hash::hash_data(b"tx3"),
        ];
        
        let root = Hash::merkle_root(&hashes);
        assert_ne!(root, Hash::ZERO);
        
        // Empty list should return zero hash
        let empty_root = Hash::merkle_root(&[]);
        assert_eq!(empty_root, Hash::ZERO);
    }
    
    #[test]
    fn test_signature_serialization() {
        let sig = Signature::new([1u8; 32], [2u8; 32], 27);
        let bytes = sig.to_bytes();
        let sig2 = Signature::from_bytes(&bytes).unwrap();
        
        assert_eq!(sig, sig2);
    }
}