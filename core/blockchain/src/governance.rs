use crate::subnet::SubnetId;
use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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

// ============================================================================
// SubnetGovernor
// ============================================================================

/// Manages subnet-level governance proposals and voting.
pub struct SubnetGovernor {
    /// All proposals, keyed by their id hash.
    pub proposals: HashMap<Hash, Proposal>,
    /// Index: subnet → list of proposal ids.
    pub proposals_by_subnet: HashMap<SubnetId, Vec<Hash>>,
    /// Number of blocks a proposal remains open for voting.
    pub voting_period: u64,
    /// Minimum active stake a proposer must hold to submit a proposal.
    pub min_proposal_stake: Amount,
}

impl SubnetGovernor {
    // ----------------------------------------------------------------
    // Constructor
    // ----------------------------------------------------------------

    /// Create a new governor.
    ///
    /// * `voting_period` — blocks a proposal is open for voting (default 1 000)
    /// * `min_proposal_stake` — minimum stake required to propose (default 10 000 ISA in base units)
    pub fn new(voting_period: u64, min_proposal_stake: Amount) -> Self {
        SubnetGovernor {
            proposals: HashMap::new(),
            proposals_by_subnet: HashMap::new(),
            voting_period,
            min_proposal_stake,
        }
    }

    // ----------------------------------------------------------------
    // Proposal creation
    // ----------------------------------------------------------------

    /// Submit a new governance proposal for `subnet_id`.
    ///
    /// Returns the proposal id (`Hash`) on success.
    ///
    /// # Errors
    /// * `InsufficientStake` — `proposer_stake` < `self.min_proposal_stake`
    /// * `InvalidProposal` — `title` or `description` is empty
    pub fn create_proposal(
        &mut self,
        subnet_id: SubnetId,
        proposer: Address,
        proposer_stake: Amount,
        title: String,
        description: String,
        action: ProposalAction,
        height: BlockHeight,
    ) -> Result<Hash, GovernanceError> {
        if proposer_stake < self.min_proposal_stake {
            return Err(GovernanceError::InsufficientStake);
        }

        if title.trim().is_empty() || description.trim().is_empty() {
            return Err(GovernanceError::InvalidProposal);
        }

        // Derive a deterministic proposal id from its content.
        let mut id_input = Vec::new();
        id_input.extend_from_slice(proposer.as_bytes());
        id_input.extend_from_slice(&(subnet_id as u8 as u64).to_le_bytes());
        id_input.extend_from_slice(title.as_bytes());
        id_input.extend_from_slice(&height.to_le_bytes());
        let id = Hash::hash_data(&id_input);

        let proposal = Proposal {
            id,
            subnet_id,
            proposer,
            title,
            description,
            action,
            votes_for: 0,
            votes_against: 0,
            voters: HashSet::new(),
            status: ProposalStatus::Active,
            created_at: height,
            voting_ends_at: height + self.voting_period,
            quorum_bps: 3000,   // 30 %
            threshold_bps: 6000, // 60 %
        };

        self.proposals_by_subnet
            .entry(subnet_id)
            .or_insert_with(Vec::new)
            .push(id);

        self.proposals.insert(id, proposal);
        Ok(id)
    }

    // ----------------------------------------------------------------
    // Voting
    // ----------------------------------------------------------------

    /// Cast a vote on `proposal_id`.
    ///
    /// * `stake_weight` — the voter's current active stake; must be > 0.
    /// * `support` — `true` = vote FOR, `false` = vote AGAINST.
    ///
    /// # Errors
    /// * `ProposalNotFound`
    /// * `VotingEnded` — `current_height` > `voting_ends_at`
    /// * `AlreadyVoted`
    /// * `NotStaker` — `stake_weight` == 0
    pub fn vote(
        &mut self,
        proposal_id: &Hash,
        voter: Address,
        stake_weight: Amount,
        support: bool,
        current_height: BlockHeight,
    ) -> Result<(), GovernanceError> {
        let proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or(GovernanceError::ProposalNotFound(*proposal_id))?;

        if current_height > proposal.voting_ends_at {
            return Err(GovernanceError::VotingEnded);
        }

        if proposal.voters.contains(&voter) {
            return Err(GovernanceError::AlreadyVoted);
        }

        if stake_weight == 0 {
            return Err(GovernanceError::NotStaker(voter));
        }

        if support {
            proposal.votes_for += stake_weight;
        } else {
            proposal.votes_against += stake_weight;
        }

        proposal.voters.insert(voter);
        Ok(())
    }

    // ----------------------------------------------------------------
    // Tallying
    // ----------------------------------------------------------------

    /// Compute the final outcome of `proposal_id` after the voting period ends.
    ///
    /// Updates the proposal's `status` in place and returns the new status.
    ///
    /// # Errors
    /// * `ProposalNotFound`
    /// * `VotingNotEnded` — `current_height` <= `voting_ends_at`
    pub fn tally(
        &mut self,
        proposal_id: &Hash,
        total_subnet_stake: Amount,
        current_height: BlockHeight,
    ) -> Result<ProposalStatus, GovernanceError> {
        let proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or(GovernanceError::ProposalNotFound(*proposal_id))?;

        if current_height <= proposal.voting_ends_at {
            return Err(GovernanceError::VotingNotEnded);
        }

        let total_votes = proposal.votes_for + proposal.votes_against;

        // Check quorum: total_votes / total_subnet_stake >= quorum_bps / 10_000
        // Rearranged to avoid floating point: total_votes * 10_000 >= quorum_bps * total_subnet_stake
        let quorum_met = if total_subnet_stake == 0 {
            false
        } else {
            total_votes
                .saturating_mul(10_000)
                >= (proposal.quorum_bps as u128).saturating_mul(total_subnet_stake)
        };

        // Check threshold: votes_for / total_votes >= threshold_bps / 10_000
        let threshold_met = if total_votes == 0 {
            false
        } else {
            proposal
                .votes_for
                .saturating_mul(10_000)
                >= (proposal.threshold_bps as u128).saturating_mul(total_votes)
        };

        let new_status = if quorum_met && threshold_met {
            ProposalStatus::Passed
        } else {
            ProposalStatus::Rejected
        };

        proposal.status = new_status.clone();
        Ok(new_status)
    }

    // ----------------------------------------------------------------
    // Execution
    // ----------------------------------------------------------------

    /// Mark a passed proposal as executed and return the action to apply.
    ///
    /// # Errors
    /// * `ProposalNotFound`
    /// * `ProposalNotPassed` — status is not `Passed`
    /// * `AlreadyExecuted` — status is already `Executed`
    pub fn execute(&mut self, proposal_id: &Hash) -> Result<ProposalAction, GovernanceError> {
        let proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or(GovernanceError::ProposalNotFound(*proposal_id))?;

        match &proposal.status {
            ProposalStatus::Executed => return Err(GovernanceError::AlreadyExecuted),
            ProposalStatus::Passed => {}
            _ => return Err(GovernanceError::ProposalNotPassed),
        }

        proposal.status = ProposalStatus::Executed;
        Ok(proposal.action.clone())
    }

    // ----------------------------------------------------------------
    // Expiry
    // ----------------------------------------------------------------

    /// Scan all proposals and mark any `Active` ones whose voting period has
    /// ended as `Expired`.
    ///
    /// This is a maintenance sweep; callers should invoke it each block.
    pub fn expire_proposals(&mut self, current_height: BlockHeight) {
        for proposal in self.proposals.values_mut() {
            if proposal.status == ProposalStatus::Active
                && current_height > proposal.voting_ends_at
            {
                proposal.status = ProposalStatus::Expired;
            }
        }
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    /// Look up a proposal by id.
    pub fn get_proposal(&self, id: &Hash) -> Option<&Proposal> {
        self.proposals.get(id)
    }

    /// Return all proposals for a given subnet (in creation order).
    pub fn get_subnet_proposals(&self, subnet_id: &SubnetId) -> Vec<&Proposal> {
        self.proposals_by_subnet
            .get(subnet_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.proposals.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return only the proposals that are currently `Active` and accepting
    /// votes (i.e., `voting_ends_at` >= `current_height`).
    pub fn get_active_proposals(
        &self,
        subnet_id: &SubnetId,
        current_height: BlockHeight,
    ) -> Vec<&Proposal> {
        self.get_subnet_proposals(subnet_id)
            .into_iter()
            .filter(|p| {
                p.status == ProposalStatus::Active && current_height <= p.voting_ends_at
            })
            .collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const ISA: Amount = 1_000_000_000_000_000_000; // 1 ISA in wei

    fn isa(n: u128) -> Amount {
        n * ISA
    }

    fn addr(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    /// Default governor: 1 000-block voting period, 10 000 ISA min stake.
    fn governor() -> SubnetGovernor {
        SubnetGovernor::new(1_000, isa(10_000))
    }

    fn make_proposal(gov: &mut SubnetGovernor, proposer: Address, height: BlockHeight) -> Hash {
        gov.create_proposal(
            SubnetId::Model,
            proposer,
            isa(10_000),
            "Test proposal".to_string(),
            "A test proposal description".to_string(),
            ProposalAction::ChangeFeeRate(300),
            height,
        )
        .unwrap()
    }

    // ----------------------------------------------------------------

    #[test]
    fn test_create_proposal() {
        let mut gov = governor();
        let proposer = addr(0x01);
        let id = make_proposal(&mut gov, proposer, 100);

        let p = gov.get_proposal(&id).unwrap();
        assert_eq!(p.subnet_id, SubnetId::Model);
        assert_eq!(p.proposer, proposer);
        assert_eq!(p.status, ProposalStatus::Active);
        assert_eq!(p.created_at, 100);
        assert_eq!(p.voting_ends_at, 1_100); // 100 + 1_000
        assert_eq!(p.quorum_bps, 3000);
        assert_eq!(p.threshold_bps, 6000);
        assert_eq!(p.votes_for, 0);
        assert_eq!(p.votes_against, 0);

        // Indexed by subnet.
        assert_eq!(gov.get_subnet_proposals(&SubnetId::Model).len(), 1);
    }

    #[test]
    fn test_vote() {
        let mut gov = governor();
        let proposer = addr(0x01);
        let voter_a = addr(0x02);
        let voter_b = addr(0x03);
        let id = make_proposal(&mut gov, proposer, 0);

        gov.vote(&id, voter_a, isa(5_000), true, 500).unwrap();
        gov.vote(&id, voter_b, isa(2_000), false, 500).unwrap();

        let p = gov.get_proposal(&id).unwrap();
        assert_eq!(p.votes_for, isa(5_000));
        assert_eq!(p.votes_against, isa(2_000));
        assert!(p.voters.contains(&voter_a));
        assert!(p.voters.contains(&voter_b));
    }

    #[test]
    fn test_double_vote_fails() {
        let mut gov = governor();
        let proposer = addr(0x01);
        let voter = addr(0x02);
        let id = make_proposal(&mut gov, proposer, 0);

        gov.vote(&id, voter, isa(1_000), true, 10).unwrap();
        let err = gov.vote(&id, voter, isa(1_000), true, 20).unwrap_err();
        assert_eq!(err, GovernanceError::AlreadyVoted);
    }

    #[test]
    fn test_vote_after_end_fails() {
        let mut gov = governor();
        let proposer = addr(0x01);
        let voter = addr(0x02);
        // Voting ends at block 1 000.
        let id = make_proposal(&mut gov, proposer, 0);

        // Try voting at block 1 001.
        let err = gov
            .vote(&id, voter, isa(1_000), true, 1_001)
            .unwrap_err();
        assert_eq!(err, GovernanceError::VotingEnded);
    }

    #[test]
    fn test_tally_passed() {
        let mut gov = governor();
        let proposer = addr(0x01);
        let id = make_proposal(&mut gov, proposer, 0);

        // Total subnet stake = 100 000 ISA.
        // Quorum needed: 30 % = 30 000 ISA.
        // Threshold: 60 % of participating votes.
        let total_stake = isa(100_000);

        // 40 000 ISA for, 5 000 ISA against → total 45 000 (> 30 000 quorum).
        // votes_for / total = 40 / 45 ≈ 88.9 % > 60 %.
        gov.vote(&id, addr(0x02), isa(40_000), true, 500).unwrap();
        gov.vote(&id, addr(0x03), isa(5_000), false, 500).unwrap();

        let status = gov.tally(&id, total_stake, 1_001).unwrap();
        assert_eq!(status, ProposalStatus::Passed);
        assert_eq!(gov.get_proposal(&id).unwrap().status, ProposalStatus::Passed);
    }

    #[test]
    fn test_tally_rejected() {
        let mut gov = governor();
        let proposer = addr(0x01);
        let id = make_proposal(&mut gov, proposer, 0);

        let total_stake = isa(100_000);

        // 40 000 for, 40 000 against → 50 % < 60 % threshold.
        gov.vote(&id, addr(0x02), isa(40_000), true, 500).unwrap();
        gov.vote(&id, addr(0x03), isa(40_000), false, 500).unwrap();

        let status = gov.tally(&id, total_stake, 1_001).unwrap();
        assert_eq!(status, ProposalStatus::Rejected);
    }

    #[test]
    fn test_tally_no_quorum() {
        let mut gov = governor();
        let proposer = addr(0x01);
        let id = make_proposal(&mut gov, proposer, 0);

        let total_stake = isa(100_000);

        // Only 5 000 ISA voted (5 % of 100 000) — well below the 30 % quorum.
        gov.vote(&id, addr(0x02), isa(5_000), true, 500).unwrap();

        let status = gov.tally(&id, total_stake, 1_001).unwrap();
        assert_eq!(status, ProposalStatus::Rejected);
    }

    #[test]
    fn test_execute() {
        let mut gov = governor();
        let proposer = addr(0x01);
        let id = make_proposal(&mut gov, proposer, 0);

        // Sufficient votes to pass.
        gov.vote(&id, addr(0x02), isa(40_000), true, 500).unwrap();
        gov.tally(&id, isa(100_000), 1_001).unwrap();

        let action = gov.execute(&id).unwrap();
        assert_eq!(action, ProposalAction::ChangeFeeRate(300));
        assert_eq!(
            gov.get_proposal(&id).unwrap().status,
            ProposalStatus::Executed
        );
    }

    #[test]
    fn test_execute_not_passed_fails() {
        let mut gov = governor();
        let proposer = addr(0x01);
        let id = make_proposal(&mut gov, proposer, 0);

        // Don't vote at all — proposal is still Active.
        let err = gov.execute(&id).unwrap_err();
        assert_eq!(err, GovernanceError::ProposalNotPassed);
    }

    #[test]
    fn test_execute_already_executed_fails() {
        let mut gov = governor();
        let proposer = addr(0x01);
        let id = make_proposal(&mut gov, proposer, 0);

        gov.vote(&id, addr(0x02), isa(40_000), true, 500).unwrap();
        gov.tally(&id, isa(100_000), 1_001).unwrap();
        gov.execute(&id).unwrap();

        let err = gov.execute(&id).unwrap_err();
        assert_eq!(err, GovernanceError::AlreadyExecuted);
    }

    #[test]
    fn test_expire_proposals() {
        let mut gov = governor();
        let id = make_proposal(&mut gov, addr(0x01), 0);

        // Before expiry — still Active.
        gov.expire_proposals(500);
        assert_eq!(
            gov.get_proposal(&id).unwrap().status,
            ProposalStatus::Active
        );

        // After voting_ends_at (1 000) — Expired.
        gov.expire_proposals(1_001);
        assert_eq!(
            gov.get_proposal(&id).unwrap().status,
            ProposalStatus::Expired
        );
    }

    #[test]
    fn test_insufficient_stake_to_propose() {
        let mut gov = governor();
        let err = gov
            .create_proposal(
                SubnetId::Tools,
                addr(0x01),
                isa(9_999), // below the 10 000 ISA minimum
                "Low stake proposal".to_string(),
                "Should fail".to_string(),
                ProposalAction::PauseSubnet,
                1,
            )
            .unwrap_err();
        assert_eq!(err, GovernanceError::InsufficientStake);
    }

    #[test]
    fn test_get_active_proposals() {
        let mut gov = governor();
        let id1 = make_proposal(&mut gov, addr(0x01), 0);   // ends at 1 000
        let id2 = make_proposal(&mut gov, addr(0x02), 500); // ends at 1 500

        // At block 1 001: id1 voting period has ended, id2 is still active.
        let active = gov.get_active_proposals(&SubnetId::Model, 1_001);
        let active_ids: Vec<Hash> = active.iter().map(|p| p.id).collect();
        assert!(!active_ids.contains(&id1));
        assert!(active_ids.contains(&id2));
    }

    #[test]
    fn test_vote_zero_stake_fails() {
        let mut gov = governor();
        let id = make_proposal(&mut gov, addr(0x01), 0);

        let err = gov
            .vote(&id, addr(0x02), 0, true, 100)
            .unwrap_err();
        assert_eq!(err, GovernanceError::NotStaker(addr(0x02)));
    }

    #[test]
    fn test_tally_before_end_fails() {
        let mut gov = governor();
        let id = make_proposal(&mut gov, addr(0x01), 0);

        // Voting ends at 1 000; try to tally at 999.
        let err = gov.tally(&id, isa(100_000), 999).unwrap_err();
        assert_eq!(err, GovernanceError::VotingNotEnded);
    }

    #[test]
    fn test_proposal_not_found() {
        let mut gov = governor();
        let fake_id = Hash::from([0xDE; 32]);

        assert!(gov.get_proposal(&fake_id).is_none());
        assert_eq!(
            gov.vote(&fake_id, addr(0x01), isa(100), true, 1).unwrap_err(),
            GovernanceError::ProposalNotFound(fake_id)
        );
        assert_eq!(
            gov.tally(&fake_id, isa(1_000), 2_000).unwrap_err(),
            GovernanceError::ProposalNotFound(fake_id)
        );
        assert_eq!(
            gov.execute(&fake_id).unwrap_err(),
            GovernanceError::ProposalNotFound(fake_id)
        );
    }
}
