use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};

// ============================================================================
// Score constants (basis points)
// ============================================================================

pub const SUCCESS_DELTA: i32 = 50;
pub const FAILURE_DELTA: i32 = -100;
pub const DISPUTE_WIN_DELTA: i32 = 200;
pub const DISPUTE_LOSS_DELTA: i32 = -300;
pub const MAX_SCORE: u32 = 10_000;

// ============================================================================
// Errors
// ============================================================================

/// Errors for the reputation system
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReputationError {
    #[error("Agent not found: {0:?}")]
    AgentNotFound(Hash),

    #[error("Invalid score: score must be in 0–10 000 basis points")]
    InvalidScore,

    #[error("Unauthorized updater: {0}")]
    UnauthorizedUpdater(Address),

    #[error("Agent already registered: {0:?}")]
    AlreadyRegistered(Hash),
}

// ============================================================================
// Supporting Types
// ============================================================================

/// Kind of event that changed an agent's reputation
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReputationEventType {
    JobCompleted,
    JobFailed,
    DisputeWon,
    DisputeLost,
    SlashPenalty,
    BonusReward,
    PeerReview,
}

/// A single scored event in an agent's reputation history
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReputationEvent {
    /// What caused the change
    pub event_type: ReputationEventType,
    /// Score delta in basis points (may be negative)
    pub delta: i32,
    /// Block at which the event occurred
    pub height: BlockHeight,
    /// Free-form description or reference
    pub details: String,
}

/// Full on-chain reputation record for an agent
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReputationRecord {
    /// Unique agent identifier
    pub agent_id: Hash,
    /// Current score in basis points (0–10 000)
    pub score: u32,
    /// Total number of interactions (success + failure)
    pub total_interactions: u64,
    /// Number of successfully completed interactions
    pub successful_interactions: u64,
    /// Number of failed interactions
    pub failed_interactions: u64,
    /// Cumulative revenue from successful interactions
    pub total_revenue: Amount,
    /// Exponential moving average of response time
    pub average_response_time_ms: u64,
    /// Block at which the record was last touched
    pub last_updated: BlockHeight,
    /// Full audit trail of score-changing events
    pub history: Vec<ReputationEvent>,
}
