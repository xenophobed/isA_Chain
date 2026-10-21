use crate::types::*;
use crate::types::{
    ComputeCapacity, ComputeUsage, DisputeType, ProviderStatus, ResourceType,
};
use serde::{Deserialize, Serialize};

/// Validator description metadata
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ValidatorDescription {
    pub moniker: String,
    pub identity: String,
    pub website: String,
    pub security_contact: String,
    pub details: String,
}

/// Validator information for staking
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ValidatorInfo {
    pub public_key: Vec<u8>,
    pub commission_rate: u32, // Basis points (10000 = 100%)
    pub min_self_delegation: Amount,
    pub description: ValidatorDescription,
}

/// Governance proposal types
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ProposalType {
    TextProposal,
    ParameterChange,
    SoftwareUpgrade,
    ValidatorSlash,
    Treasury,
}

/// Vote types for governance
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum VoteType {
    Yes,
    No,
    Abstain,
    NoWithVeto,
}

/// Transaction data based on transaction type
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TransactionData {
    /// Transfer tokens to another address
    Transfer {
        to: Address,
        amount: Amount,
        data: Vec<u8>, // Optional data payload
    },
    
    /// Deploy a new smart contract
    ContractDeploy {
        bytecode: Vec<u8>,
        constructor_args: Vec<u8>,
        salt: Option<Hash>, // For deterministic deployment
    },
    
    /// Call a smart contract function
    ContractCall {
        contract: Address,
        function_selector: [u8; 4],
        args: Vec<u8>,
        value: Amount, // Value to send with call
    },
    
    /// Stake tokens to become a validator
    Stake {
        amount: Amount,
        validator_info: ValidatorInfo,
    },
    
    /// Unstake tokens and stop being a validator
    Unstake {
        amount: Amount,
        validator: Address,
    },
    
    /// Delegate tokens to a validator
    Delegate {
        validator: Address,
        amount: Amount,
    },
    
    /// Undelegate tokens from a validator
    Undelegate {
        validator: Address,
        amount: Amount,
    },
    
    /// Submit a governance proposal
    GovernanceProposal {
        title: String,
        description: String,
        proposal_type: ProposalType,
        initial_deposit: Amount,
    },
    
    /// Vote on a governance proposal
    GovernanceVote {
        proposal_id: u64,
        vote: VoteType,
    },
    
    /// Cross-chain bridge transaction
    Bridge {
        destination_chain: ChainId,
        recipient: Vec<u8>, // Address on destination chain
        amount: Amount,
        token: Address,     // Token contract address (or zero for native)
    },

    // ========== Agent-native Economy ==========
    /// Purchase credits by burning ISA
    CreditPurchase {
        buyer: Address,
        isa_amount: Amount,
    },

    /// Spend credits on a service
    CreditSpend {
        spender: Address,
        amount: Amount,
        service: String,
    },

    /// Agent wallet autonomous spend
    AgentWalletSpend {
        wallet: Address,
        amount: Amount,
        service: String,
    },

    // ========== Compute Marketplace ==========
    /// Register as a compute provider
    ComputeRegister {
        resource_types: Vec<ResourceType>,
        capacity: ComputeCapacity,
        price_per_hour: Amount,
        min_duration_secs: u64,
        max_duration_secs: u64,
        stake: Amount,
        region: Option<String>,
        endpoint: String,
    },

    /// Update provider info
    ComputeUpdateProvider {
        resource_types: Option<Vec<ResourceType>>,
        price_per_hour: Option<Amount>,
        status: Option<ProviderStatus>,
        region: Option<String>,
        endpoint: Option<String>,
    },

    /// Provider exit (begin unstaking)
    ComputeProviderExit,

    /// Request compute resources (creates a job)
    ComputeRequest {
        job_id: Hash,
        provider: Option<Address>, // None = open market matching
        resource_type: ResourceType,
        capacity: ComputeCapacity,
        max_price_per_hour: Amount,
        duration_secs: u64,
        escrow_amount: Amount,
    },

    /// Provider accepts a job
    ComputeAccept {
        job_id: Hash,
        price_per_hour: Amount,
    },

    /// Mark job as started
    ComputeStart {
        job_id: Hash,
    },

    /// Settle a completed job
    ComputeSettle {
        job_id: Hash,
        usage: ComputeUsage,
    },

    /// Cancel a pending job
    ComputeCancel {
        job_id: Hash,
    },

    /// Open a dispute on a job
    ComputeDispute {
        dispute_id: Hash,
        job_id: Hash,
        dispute_type: DisputeType,
        evidence_hash: Hash,
    },

    /// Resolve a dispute
    ComputeResolveDispute {
        dispute_id: Hash,
        winner: Address,
        user_refund: Amount,
        provider_payment: Amount,
        slash_amount: Amount,
    },

    /// Settlement finalization
    Settlement {
        user: Address,
        provider: Address,
        gross_amount: Amount,
        fee_amount: Amount,
    },
}
