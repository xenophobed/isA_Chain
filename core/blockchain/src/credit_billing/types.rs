use crate::settlement::ServiceType;
use crate::types::{Address, Amount, BlockHeight, Hash, Timestamp};
use serde::{Deserialize, Serialize};

// ============================================================================
// CreditBillingRecord
// ============================================================================

/// A single credit-billing event for a user consuming a service.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreditBillingRecord {
    /// Unique identifier for this billing record
    pub id: Hash,
    /// User address that was charged
    pub user: Address,
    /// Which service was consumed
    pub service: ServiceType,
    /// Number of credits deducted
    pub credits_charged: Amount,
    /// Human-readable description of the usage
    pub description: String,
    /// Block height at which this charge occurred
    pub height: BlockHeight,
    /// Unix timestamp (ms) at which this charge occurred
    pub timestamp: Timestamp,
}

// ============================================================================
// ServiceUsageSummary
// ============================================================================

/// Aggregate usage statistics for a single service type.
#[derive(Clone, Debug)]
pub struct ServiceUsageSummary {
    /// The service this summary covers
    pub service: ServiceType,
    /// Total credits charged across all users
    pub total_credits: Amount,
    /// Total number of billing requests
    pub total_requests: u64,
    /// Average credits per request (0 when no requests)
    pub average_cost: Amount,
}

// ============================================================================
// CreditBillingError
// ============================================================================

/// Errors that can occur during credit billing operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CreditBillingError {
    #[error("Insufficient credits: available {available}, required {required}")]
    InsufficientCredits { available: Amount, required: Amount },

    #[error("Service is not enabled for billing")]
    ServiceNotEnabled,

    #[error("Rate limit exceeded for user/service combination")]
    RateLimitExceeded,

    #[error("Invalid amount: must be greater than zero")]
    InvalidAmount,

    #[error("User not found: {0:?}")]
    UserNotFound(Address),
}
