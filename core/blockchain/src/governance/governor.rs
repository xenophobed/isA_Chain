use super::types::{GovernanceError, Proposal, ProposalAction, ProposalStatus};
use crate::subnet::SubnetId;
use crate::types::{Address, Amount, BlockHeight, Hash};
use std::collections::{HashMap, HashSet};

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
    #[allow(clippy::too_many_arguments)]
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
            .or_default()
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
