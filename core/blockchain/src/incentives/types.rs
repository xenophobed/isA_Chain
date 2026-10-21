use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};

// ============================================================================
// Enums
// ============================================================================

/// Reason a reward was granted to a contributor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RewardReason {
    BugBounty,
    FeatureDevelopment,
    Documentation,
    SecurityAudit,
    CommunityContribution,
    ToolDevelopment,
    Custom(String),
}

/// Lifecycle state of an incentive program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgramStatus {
    Active,
    Paused,
    Expired,
    Exhausted,
}

// ============================================================================
// Structs
// ============================================================================

/// A single reward disbursement within a program.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reward {
    pub recipient: Address,
    pub amount: Amount,
    pub reason: RewardReason,
    pub height: BlockHeight,
}

/// A treasury-funded developer incentive program.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IncentiveProgram {
    pub id: Hash,
    pub name: String,
    /// Total budget allocated from treasury.
    pub budget: Amount,
    /// Amount already disbursed.
    pub spent: Amount,
    pub rewards: Vec<Reward>,
    pub status: ProgramStatus,
    pub created_at: BlockHeight,
    pub expires_at: BlockHeight,
    /// Address authorised to award rewards for this program.
    pub admin: Address,
}

impl IncentiveProgram {
    pub(crate) fn remaining(&self) -> Amount {
        self.budget.saturating_sub(self.spent)
    }
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IncentiveError {
    #[error("Program not found: {0:?}")]
    ProgramNotFound(Hash),

    #[error("Program is not active")]
    ProgramNotActive,

    #[error("Budget exhausted: remaining {remaining}, requested {requested}")]
    BudgetExhausted { remaining: Amount, requested: Amount },

    #[error("Program has expired")]
    ProgramExpired,

    #[error("Unauthorized admin: {0:?}")]
    UnauthorizedAdmin(Address),

    #[error("Amount must be greater than zero")]
    InvalidAmount,

    #[error("A program with this ID already exists")]
    DuplicateProgram,
}
