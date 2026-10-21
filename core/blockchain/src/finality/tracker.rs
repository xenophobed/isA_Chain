use super::types::*;
use crate::types::{Amount, BlockHeight, Hash};
use std::collections::HashMap;

/// Tracks validator votes and computes finality for blocks.
pub struct FinalityTracker {
    /// Per-block finality state, keyed by block hash
    pub blocks: HashMap<Hash, BlockFinality>,
    /// Highest finalized block height
    pub finalized_height: BlockHeight,
    /// Hash of the highest finalized block
    pub finalized_hash: Hash,
    /// Quorum threshold in basis points (default 6667 ≈ 2/3 + 1)
    pub quorum_threshold_bps: u32,
    /// Total stake power of the active validator set
    pub total_validator_power: Amount,
}

impl FinalityTracker {
    /// Create a new [`FinalityTracker`] with the given quorum threshold.
    ///
    /// `quorum_bps` is expressed in basis points (10000 = 100%).
    /// For a 2/3+ supermajority, pass `6667`.
    pub fn new(quorum_bps: u32) -> Self {
        FinalityTracker {
            blocks: HashMap::new(),
            finalized_height: 0,
            finalized_hash: Hash::ZERO,
            quorum_threshold_bps: quorum_bps,
            total_validator_power: 0,
        }
    }

    /// Register a block so that votes can be collected for it.
    pub fn register_block(
        &mut self,
        hash: Hash,
        height: BlockHeight,
    ) -> Result<(), FinalityError> {
        self.blocks.entry(hash).or_insert_with(|| BlockFinality {
            hash,
            height,
            votes: Vec::new(),
            vote_power: 0,
            status: FinalityStatus::Pending,
        });
        Ok(())
    }

    /// Add a validator vote for a block.
    ///
    /// Returns the updated [`FinalityStatus`] for the block, or an error if
    /// the vote is invalid (duplicate, unknown block, etc.).
    pub fn add_vote(
        &mut self,
        vote: FinalityVote,
        voter_power: Amount,
    ) -> Result<FinalityStatus, FinalityError> {
        let hash = vote.block_hash;

        // Ensure block is registered
        if !self.blocks.contains_key(&hash) {
            return Err(FinalityError::BlockNotFound(hash));
        }

        {
            let block = self.blocks.get_mut(&hash).unwrap();

            // Reject already-finalized blocks from receiving new votes
            if block.status == FinalityStatus::Finalized {
                return Err(FinalityError::AlreadyFinalized(hash));
            }

            // Reject duplicate votes from the same voter at the same height
            let already_voted = block.votes.iter().any(|v| v.voter == vote.voter);
            if already_voted {
                return Err(FinalityError::DuplicateVote {
                    voter: vote.voter,
                    height: vote.height,
                });
            }

            // Accumulate power and record vote
            block.vote_power = block.vote_power.saturating_add(voter_power);
            block.votes.push(vote);
        }

        // Re-borrow immutably to check quorum, then mutably to update status.
        // Split into two steps to satisfy the borrow checker.
        let quorum_reached = {
            let block = self.blocks.get(&hash).unwrap();
            block.status == FinalityStatus::Pending && self.check_quorum_inner(block)
        };

        if quorum_reached {
            self.blocks.get_mut(&hash).unwrap().status = FinalityStatus::Justified;
        }

        Ok(self.blocks[&hash].status.clone())
    }

    /// Return the current [`FinalityStatus`] for a block, if known.
    pub fn get_status(&self, hash: &Hash) -> Option<&FinalityStatus> {
        self.blocks.get(hash).map(|b| &b.status)
    }

    /// Return `true` if the block has been finalized.
    pub fn is_finalized(&self, hash: &Hash) -> bool {
        matches!(self.get_status(hash), Some(FinalityStatus::Finalized))
    }

    /// Return the highest finalized block height.
    pub fn get_finalized_height(&self) -> BlockHeight {
        self.finalized_height
    }

    /// Return the hash of the highest finalized block.
    pub fn get_finalized_hash(&self) -> Hash {
        self.finalized_hash
    }

    /// Update the total validator power used in quorum calculations.
    pub fn set_total_power(&mut self, power: Amount) {
        self.total_validator_power = power;
    }

    /// Return `true` when the block's accumulated vote power meets the quorum.
    ///
    /// Formula: `vote_power * 10000 / total_validator_power >= quorum_threshold_bps`
    pub fn check_quorum(&self, hash: &Hash) -> bool {
        match self.blocks.get(hash) {
            Some(block) => self.check_quorum_inner(block),
            None => false,
        }
    }

    /// Internal quorum check that operates directly on a [`BlockFinality`].
    fn check_quorum_inner(&self, block: &BlockFinality) -> bool {
        if self.total_validator_power == 0 {
            return false;
        }
        // Use u128 arithmetic to avoid overflow: vote_power * 10000
        let scaled = block
            .vote_power
            .saturating_mul(10_000)
            / self.total_validator_power;
        scaled >= self.quorum_threshold_bps as u128
    }

    /// Mark a block as [`FinalityStatus::Finalized`] and update the highest
    /// finalized height.
    ///
    /// The block must already be [`FinalityStatus::Justified`].
    pub fn finalize(&mut self, hash: &Hash) -> Result<(), FinalityError> {
        let block = self
            .blocks
            .get_mut(hash)
            .ok_or(FinalityError::BlockNotFound(*hash))?;

        if block.status == FinalityStatus::Finalized {
            return Err(FinalityError::AlreadyFinalized(*hash));
        }

        block.status = FinalityStatus::Finalized;

        // Update tracker-level finalized height / hash
        if block.height >= self.finalized_height {
            self.finalized_height = block.height;
            self.finalized_hash = *hash;
        }

        Ok(())
    }

    /// Return an immutable reference to the [`BlockFinality`] record for a block.
    pub fn get_block_finality(&self, hash: &Hash) -> Option<&BlockFinality> {
        self.blocks.get(hash)
    }

    /// Remove finality records for all blocks strictly below `height`.
    ///
    /// This is safe to call after finality has advanced; old data is no longer
    /// needed and can be pruned to bound memory usage.
    pub fn prune_below(&mut self, height: BlockHeight) {
        self.blocks.retain(|_, b| b.height >= height);
    }
}
