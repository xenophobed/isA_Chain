use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};

/// Errors for the agent registry
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("Agent not found: {0:?}")]
    AgentNotFound(Hash),

    #[error("Agent already registered: {0:?}")]
    AgentAlreadyRegistered(Hash),

    #[error("Unauthorized owner: {0}")]
    UnauthorizedOwner(Address),

    #[error("Unauthorized admin: {0}")]
    UnauthorizedAdmin(Address),

    #[error("Invalid name: agent name must not be empty")]
    InvalidName,

    #[error("No capabilities: agent must have at least one capability")]
    NoCapabilities,

    #[error("Agent is suspended and cannot be modified")]
    AgentSuspended,
}

/// Capabilities an agent can advertise
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentCapability {
    TextGeneration,
    CodeGeneration,
    ImageGeneration,
    DataAnalysis,
    WebBrowsing,
    ToolUse,
    MultiModal,
    Reasoning,
    Custom(String),
}

/// Fee structure for invoking an agent
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentPricing {
    /// Flat per-request fee in credits
    pub base_fee: Amount,
    /// Per-token fee (for LLM agents)
    pub per_token_fee: Amount,
    /// Per-second compute fee
    pub per_second_fee: Amount,
    /// Minimum charge per invocation
    pub minimum_charge: Amount,
}

/// Lifecycle status of a registered agent
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Active,
    Inactive,
    Suspended,
    Deregistered,
}

/// Full on-chain registration record for an agent
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentRegistration {
    /// Unique agent identifier
    pub agent_id: Hash,
    /// Owner / deployer of this agent
    pub owner: Address,
    /// Agent's own wallet address
    pub wallet: Address,
    /// Human-readable name
    pub name: String,
    /// Description of what the agent does
    pub description: String,
    /// Advertised capabilities
    pub capabilities: Vec<AgentCapability>,
    /// Pricing structure
    pub pricing: AgentPricing,
    /// Reputation score in basis points (0–10 000)
    pub reputation_score: u32,
    /// Total job count
    pub total_jobs: u64,
    /// Successful job count
    pub successful_jobs: u64,
    /// Total revenue earned in credits
    pub total_revenue: Amount,
    /// Current lifecycle status
    pub status: AgentStatus,
    /// Block at which the agent was first registered
    pub registered_at: BlockHeight,
    /// Block of last job or status change
    pub last_active: BlockHeight,
    /// Extensible key-value metadata
    pub metadata: std::collections::HashMap<String, String>,
}
