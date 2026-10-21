use crate::settlement::ServiceType;
use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};

// ============================================================================
// DelegationStatus
// ============================================================================

/// Lifecycle state of a budget delegation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelegationStatus {
    /// Delegation is active; agent may spend.
    Active,
    /// Temporarily paused by the delegator; spending blocked.
    Paused,
    /// Total budget has been fully consumed.
    Exhausted,
    /// Delegation passed its expiry block height.
    Expired,
    /// Explicitly cancelled by the delegator.
    Revoked,
}

// ============================================================================
// BudgetDelegation
// ============================================================================

/// A record authorising an agent to spend on behalf of a human delegator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BudgetDelegation {
    /// Unique identifier for this delegation.
    pub id: Hash,
    /// Human address that created the delegation.
    pub delegator: Address,
    /// Agent wallet that may draw from this budget.
    pub agent: Address,
    /// Total credits authorised by the delegator.
    pub total_budget: Amount,
    /// Amount already spent against this delegation.
    pub spent: Amount,
    /// Maximum credits that can be spent in a single transaction.
    pub per_tx_limit: Amount,
    /// Restrict spending to specific service types; `None` means all services.
    pub allowed_services: Option<Vec<ServiceType>>,
    /// Block height after which the delegation is no longer valid.
    pub expires_at: BlockHeight,
    /// Current lifecycle status.
    pub status: DelegationStatus,
    /// Block height at which the delegation was created.
    pub created_at: BlockHeight,
}

// ============================================================================
// BudgetError
// ============================================================================

/// Errors that can arise from budget delegation operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BudgetError {
    #[error("Delegation not found: {0}")]
    DelegationNotFound(Hash),

    #[error("Budget exhausted")]
    BudgetExhausted,

    #[error("Exceeds per-transaction limit: limit {limit}, requested {requested}")]
    ExceedsPerTxLimit { limit: Amount, requested: Amount },

    #[error("Exceeds remaining budget: remaining {remaining}, requested {requested}")]
    ExceedsBudget { remaining: Amount, requested: Amount },

    #[error("Delegation has expired")]
    DelegationExpired,

    #[error("Delegation is not active")]
    DelegationNotActive,

    #[error("Unauthorized delegator: {0}")]
    UnauthorizedDelegator(Address),

    #[error("Service type not allowed by this delegation")]
    ServiceNotAllowed,

    #[error("Invalid amount: must be greater than zero")]
    InvalidAmount,

    #[error("A delegation between this delegator and agent already exists")]
    DuplicateDelegation,
}
