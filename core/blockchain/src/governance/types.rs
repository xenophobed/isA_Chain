use crate::subnet::SubnetId;
use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ============================================================================
// ProposalAction
// ============================================================================

/// The on-chain action to execute if a proposal passes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalAction {
    /// Change the subnet's protocol fee rate (in basis points).
    ChangeFeeRate(u32),
    /// Change the minimum stake required to participate as a provider.
    ChangeMinStake(Amount),
    /// Change the hard cap on registered providers.
    ChangeMaxProviders(usize),
    /// Change the subnet's emission weight (in basis points).
    ChangeEmissionWeight(u32),
    /// Pause all activity in the subnet.
    PauseSubnet,
    /// Resume a previously paused subnet.
    ResumeSubnet,
    /// Arbitrary governance action encoded as a human-readable string.
    Custom(String),
}

// ============================================================================
// ProposalStatus
// ============================================================================

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    /// Voting is open.
    Active,
    /// Quorum reached and threshold met — waiting for execution.
    Passed,
    /// Voting ended; either quorum not reached or threshold not met.
    Rejected,
    /// `execute()` was called after the proposal passed.
    Executed,
    /// Voting period ended before quorum was reached (alias for Rejected,
    /// tracked separately for UI clarity).
    Expired,
}

// ============================================================================
// GovernanceError
// ============================================================================

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GovernanceError {
    #[error("Proposal not found: {0:?}")]
    ProposalNotFound(Hash),

    #[error("Voting period has already ended")]
    VotingEnded,

    #[error("Voting period has not yet ended")]
    VotingNotEnded,

    #[error("Address has already voted on this proposal")]
    AlreadyVoted,

    #[error("Address {0:?} has no stake in the subnet and cannot vote")]
    NotStaker(Address),

    #[error("Stake is below the minimum required to create a proposal")]
    InsufficientStake,

    #[error("Proposal has not passed")]
    ProposalNotPassed,

    #[error("Proposal has already been executed")]
    AlreadyExecuted,

    #[error("Proposal is invalid (e.g., empty title or description)")]
    InvalidProposal,
}

// ============================================================================
// Proposal
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Proposal {
    /// Unique identifier derived from proposal content hash.
    pub id: Hash,
    /// Which subnet this proposal governs.
    pub subnet_id: SubnetId,
    /// Address that submitted the proposal.
    pub proposer: Address,
    /// Short human-readable title.
    pub title: String,
    /// Longer description / rationale.
    pub description: String,
    /// The action to execute on-chain if the proposal passes.
    pub action: ProposalAction,
    /// Accumulated stake weight voting in favour.
    pub votes_for: Amount,
    /// Accumulated stake weight voting against.
    pub votes_against: Amount,
    /// Set of addresses that have cast a vote (prevents double-voting).
    pub voters: HashSet<Address>,
    /// Current lifecycle status.
    pub status: ProposalStatus,
    /// Block at which the proposal was created.
    pub created_at: BlockHeight,
    /// Block after which no new votes are accepted.
    pub voting_ends_at: BlockHeight,
    /// Minimum share of total subnet stake that must vote for the result to be
    /// valid, expressed in basis points (e.g. 3000 = 30 %).
    pub quorum_bps: u32,
    /// Minimum share of *participating* stake that must vote FOR for the
    /// proposal to pass, expressed in basis points (e.g. 6000 = 60 %).
    pub threshold_bps: u32,
}
