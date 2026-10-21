use crate::settlement::ServiceType;
use crate::types::{Address, Amount, Timestamp};
use serde::{Deserialize, Serialize};

// ============================================================================
// BillingMetadata
// ============================================================================

/// Service-specific metadata attached to a billing hook event.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BillingMetadata {
    /// isA_Model — model inference billing metadata
    ModelInference {
        model_id: String,
        input_tokens: u64,
        output_tokens: u64,
        latency_ms: u64,
    },
    /// isA_MCP — tool execution billing metadata
    ToolExecution {
        tool_name: String,
        execution_time_ms: u64,
        success: bool,
    },
    /// isA_OS — compute usage billing metadata
    ComputeUsage {
        resource_type: String,
        compute_seconds: u64,
        memory_mb_seconds: u64,
    },
    /// isA_Data — storage billing metadata
    Storage {
        bytes_stored: u64,
        duration_secs: u64,
    },
    /// isA_Agent — agent runtime billing metadata
    AgentRuntime {
        agent_id: String,
        runtime_secs: u64,
        actions_count: u64,
    },
    /// Catch-all for extensibility
    Generic { description: String },
}

impl BillingMetadata {
    /// Returns the `ServiceType` that corresponds to this metadata variant.
    pub fn service_type(&self) -> ServiceType {
        match self {
            BillingMetadata::ModelInference { .. } => ServiceType::ModelInference,
            BillingMetadata::ToolExecution { .. } => ServiceType::ToolExecution,
            BillingMetadata::ComputeUsage { .. } => ServiceType::ComputeUsage,
            BillingMetadata::Storage { .. } => ServiceType::Storage,
            BillingMetadata::AgentRuntime { .. } => ServiceType::AgentRuntime,
            BillingMetadata::Generic { .. } => ServiceType::Custom("generic".to_string()),
        }
    }
}

// ============================================================================
// BillingHookEvent
// ============================================================================

/// An event emitted by a service (isA_Model, isA_MCP, isA_OS, …) that drives
/// an on-chain settlement via the `SettlementBridge`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillingHookEvent {
    /// Unique event ID in UUID format.
    pub event_id: String,
    /// Which isA service generated this event.
    pub service: ServiceType,
    /// Address of the user being charged.
    pub user: Address,
    /// Provider's on-chain wallet address (receives the net payment).
    pub provider_wallet: Address,
    /// Billed amount in credits.
    pub amount: Amount,
    /// Unix timestamp (milliseconds) when the event was generated.
    pub timestamp: Timestamp,
    /// Service-specific billing metadata.
    pub metadata: BillingMetadata,
}

// ============================================================================
// BillingConfig
// ============================================================================

/// Per-service billing configuration used by `BillingCalculator`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillingConfig {
    /// Which isA service this config applies to.
    pub service: ServiceType,
    /// Provider's on-chain wallet address.
    pub provider_wallet: Address,
    /// Base rate charged per billing event (in credits).
    pub base_rate: Amount,
    /// Additional rate per token (for model inference).
    pub rate_per_token: Amount,
    /// Additional rate per second of compute/execution time.
    pub rate_per_second: Amount,
    /// Minimum charge per event (floor applied after formula).
    pub min_charge: Amount,
    /// Whether billing is active for this service.
    pub enabled: bool,
}

// ============================================================================
// BillingError
// ============================================================================

/// Errors returned by `BillingCalculator`.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BillingError {
    #[error("service not configured")]
    ServiceNotConfigured,

    #[error("metadata does not match configured service")]
    InvalidMetadata,

    #[error("usage metrics are zero — nothing to bill")]
    ZeroUsage,

    #[error("billing is disabled for this service")]
    BillingDisabled,
}
