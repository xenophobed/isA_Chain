//! Block finality gadget for isA Chain.
//!
//! Tracks validator votes on blocks and determines when blocks become
//! finalized (irreversible). Uses a 2/3+ supermajority quorum model similar
//! to Casper FFG / Tendermint.

use crate::types::{Address, Amount, BlockHeight, Hash, Signature};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// FinalityVote
// ============================================================================

/// A validator's vote attesting to a block at a given height and round.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalityVote {
    /// Validator address casting the vote
    pub voter: Address,
    /// Hash of the block being voted on
    pub block_hash: Hash,
    /// Block height
    pub height: BlockHeight,
    /// Consensus round in which the vote was cast
    pub round: u32,
    /// Validator's signature over (block_hash, height, round)
    pub signature: Signature,
}

// ============================================================================
// FinalityStatus
// ============================================================================

/// Lifecycle status of a block's finality.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinalityStatus {
    /// Block exists but has not yet accumulated sufficient votes.
    Pending,
    /// 2/3+ of total validator power has voted for this block.
    Justified,
    /// Justified AND every ancestor back to genesis is also justified.
    Finalized,
    /// This block is on a fork that lost; it will never be finalized.
    Orphaned,
}

// ============================================================================
// BlockFinality
// ============================================================================

/// Per-block finality state tracked by [`FinalityTracker`].
#[derive(Clone, Debug)]
pub struct BlockFinality {
    /// Block hash
    pub hash: Hash,
    /// Block height
    pub height: BlockHeight,
    /// All votes received for this block
    pub votes: Vec<FinalityVote>,
    /// Total stake weight of all voters
    pub vote_power: Amount,
    /// Current finality status
    pub status: FinalityStatus,
}

// ============================================================================
// FinalityError
// ============================================================================

/// Errors that can occur during finality tracking.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FinalityError {
    #[error("Block not found: {0}")]
    BlockNotFound(Hash),

    #[error("Duplicate vote from {voter:?} at height {height}")]
    DuplicateVote { voter: Address, height: BlockHeight },

    #[error("Insufficient power: required {required}, actual {actual}")]
    InsufficientPower { required: Amount, actual: Amount },

    #[error("Invalid round")]
    InvalidRound,

    #[error("Block already finalized: {0}")]
    AlreadyFinalized(Hash),

    #[error("Voter is not a registered validator: {0}")]
    VoterNotValidator(Address),
}

// ============================================================================
// FinalityTracker
// ============================================================================

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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Signature;

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    fn make_address(byte: u8) -> Address {
        let mut bytes = [0u8; 20];
        bytes[0] = byte;
        Address::from(bytes)
    }

    fn make_hash(byte: u8) -> Hash {
        let mut bytes = [0u8; 32];
        bytes[0] = byte;
        Hash::from(bytes)
    }

    fn dummy_sig() -> Signature {
        Signature::new([1u8; 32], [2u8; 32], 27)
    }

    fn make_vote(voter: Address, block_hash: Hash, height: BlockHeight, round: u32) -> FinalityVote {
        FinalityVote {
            voter,
            block_hash,
            height,
            round,
            signature: dummy_sig(),
        }
    }

    // -------------------------------------------------------------------------
    // Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_register_block() {
        let mut tracker = FinalityTracker::new(6667);
        let hash = make_hash(1);
        assert!(tracker.register_block(hash, 1).is_ok());
        let bf = tracker.get_block_finality(&hash).unwrap();
        assert_eq!(bf.height, 1);
        assert_eq!(bf.status, FinalityStatus::Pending);
        assert!(bf.votes.is_empty());
    }

    #[test]
    fn test_add_vote() {
        let mut tracker = FinalityTracker::new(6667);
        tracker.set_total_power(100);
        let hash = make_hash(1);
        tracker.register_block(hash, 1).unwrap();

        let vote = make_vote(make_address(1), hash, 1, 0);
        let status = tracker.add_vote(vote, 30).unwrap();
        // 30/100 = 30% — below quorum
        assert_eq!(status, FinalityStatus::Pending);

        let bf = tracker.get_block_finality(&hash).unwrap();
        assert_eq!(bf.votes.len(), 1);
        assert_eq!(bf.vote_power, 30);
    }

    #[test]
    fn test_duplicate_vote_rejected() {
        let mut tracker = FinalityTracker::new(6667);
        tracker.set_total_power(100);
        let hash = make_hash(1);
        tracker.register_block(hash, 1).unwrap();

        let voter = make_address(1);
        let vote1 = make_vote(voter, hash, 1, 0);
        let vote2 = make_vote(voter, hash, 1, 0);

        tracker.add_vote(vote1, 30).unwrap();
        let err = tracker.add_vote(vote2, 30).unwrap_err();
        assert_eq!(err, FinalityError::DuplicateVote { voter, height: 1 });
    }

    #[test]
    fn test_quorum_reached() {
        let mut tracker = FinalityTracker::new(6667);
        tracker.set_total_power(100);
        let hash = make_hash(1);
        tracker.register_block(hash, 1).unwrap();

        // Need ≥ 66.67% — add three voters totalling 70
        for i in 1u8..=3 {
            let vote = make_vote(make_address(i), hash, 1, 0);
            tracker.add_vote(vote, 23 + i as u128).unwrap(); // 24+25+26 = 75
        }

        let status = tracker.get_status(&hash).unwrap();
        assert_eq!(*status, FinalityStatus::Justified);
        assert!(tracker.check_quorum(&hash));
    }

    #[test]
    fn test_quorum_not_reached() {
        let mut tracker = FinalityTracker::new(6667);
        tracker.set_total_power(100);
        let hash = make_hash(1);
        tracker.register_block(hash, 1).unwrap();

        // Add only 50% power
        let vote = make_vote(make_address(1), hash, 1, 0);
        tracker.add_vote(vote, 50).unwrap();

        assert_eq!(*tracker.get_status(&hash).unwrap(), FinalityStatus::Pending);
        assert!(!tracker.check_quorum(&hash));
    }

    #[test]
    fn test_finalize_block() {
        let mut tracker = FinalityTracker::new(6667);
        tracker.set_total_power(100);
        let hash = make_hash(1);
        tracker.register_block(hash, 5).unwrap();

        // Reach quorum first
        for i in 1u8..=3 {
            let vote = make_vote(make_address(i), hash, 5, 0);
            tracker.add_vote(vote, 25).unwrap();
        }
        assert_eq!(*tracker.get_status(&hash).unwrap(), FinalityStatus::Justified);

        // Finalize
        tracker.finalize(&hash).unwrap();
        assert_eq!(*tracker.get_status(&hash).unwrap(), FinalityStatus::Finalized);
        assert!(tracker.is_finalized(&hash));
        assert_eq!(tracker.get_finalized_height(), 5);
        assert_eq!(tracker.get_finalized_hash(), hash);
    }

    #[test]
    fn test_already_finalized() {
        let mut tracker = FinalityTracker::new(6667);
        tracker.set_total_power(100);
        let hash = make_hash(1);
        tracker.register_block(hash, 1).unwrap();

        // Force justify and finalize
        let vote = make_vote(make_address(1), hash, 1, 0);
        tracker.add_vote(vote, 100).unwrap();
        tracker.finalize(&hash).unwrap();

        // Attempting to finalize again should error
        let err = tracker.finalize(&hash).unwrap_err();
        assert_eq!(err, FinalityError::AlreadyFinalized(hash));

        // Adding a vote to an already-finalized block should also error
        let vote2 = make_vote(make_address(2), hash, 1, 0);
        let err2 = tracker.add_vote(vote2, 10).unwrap_err();
        assert_eq!(err2, FinalityError::AlreadyFinalized(hash));
    }

    #[test]
    fn test_finalized_height_updates() {
        let mut tracker = FinalityTracker::new(6667);
        tracker.set_total_power(100);

        // Finalize block at height 3
        let hash3 = make_hash(3);
        tracker.register_block(hash3, 3).unwrap();
        let v = make_vote(make_address(1), hash3, 3, 0);
        tracker.add_vote(v, 100).unwrap();
        tracker.finalize(&hash3).unwrap();
        assert_eq!(tracker.get_finalized_height(), 3);

        // Finalize block at height 7 — should update
        let hash7 = make_hash(7);
        tracker.register_block(hash7, 7).unwrap();
        let v2 = make_vote(make_address(2), hash7, 7, 0);
        tracker.add_vote(v2, 100).unwrap();
        tracker.finalize(&hash7).unwrap();
        assert_eq!(tracker.get_finalized_height(), 7);
        assert_eq!(tracker.get_finalized_hash(), hash7);
    }

    #[test]
    fn test_prune_below() {
        let mut tracker = FinalityTracker::new(6667);

        // Register blocks at heights 1–5
        for i in 1u8..=5 {
            let hash = make_hash(i);
            tracker.register_block(hash, i as BlockHeight).unwrap();
        }
        assert_eq!(tracker.blocks.len(), 5);

        // Prune blocks below height 3 (removes heights 1 and 2)
        tracker.prune_below(3);
        assert_eq!(tracker.blocks.len(), 3);

        // Heights 3, 4, 5 should still be present
        for i in 3u8..=5 {
            assert!(tracker.get_block_finality(&make_hash(i)).is_some());
        }
        // Heights 1 and 2 should be gone
        for i in 1u8..=2 {
            assert!(tracker.get_block_finality(&make_hash(i)).is_none());
        }
    }

    #[test]
    fn test_get_status() {
        let mut tracker = FinalityTracker::new(6667);
        let hash = make_hash(1);

        // Unknown block → None
        assert!(tracker.get_status(&hash).is_none());

        tracker.register_block(hash, 1).unwrap();
        assert_eq!(*tracker.get_status(&hash).unwrap(), FinalityStatus::Pending);
    }

    #[test]
    fn test_quorum_threshold_math() {
        // Verify boundary: exactly 6667 bps (66.67%)
        let mut tracker = FinalityTracker::new(6667);
        tracker.set_total_power(10_000);
        let hash = make_hash(1);
        tracker.register_block(hash, 1).unwrap();

        // 6666 / 10000 = 6666 bps — just below quorum
        let v1 = make_vote(make_address(1), hash, 1, 0);
        tracker.add_vote(v1, 6666).unwrap();
        assert_eq!(*tracker.get_status(&hash).unwrap(), FinalityStatus::Pending);

        // Add 1 more unit → 6667 / 10000 = 6667 bps — exactly at quorum
        let v2 = make_vote(make_address(2), hash, 1, 0);
        tracker.add_vote(v2, 1).unwrap();
        assert_eq!(*tracker.get_status(&hash).unwrap(), FinalityStatus::Justified);
    }

    #[test]
    fn test_block_not_found_on_vote() {
        let mut tracker = FinalityTracker::new(6667);
        tracker.set_total_power(100);
        let hash = make_hash(99);

        let vote = make_vote(make_address(1), hash, 1, 0);
        let err = tracker.add_vote(vote, 50).unwrap_err();
        assert_eq!(err, FinalityError::BlockNotFound(hash));
    }

    #[test]
    fn test_zero_total_power_no_quorum() {
        let mut tracker = FinalityTracker::new(6667);
        // total_validator_power left at 0
        let hash = make_hash(1);
        tracker.register_block(hash, 1).unwrap();
        let vote = make_vote(make_address(1), hash, 1, 0);
        tracker.add_vote(vote, 1000).unwrap();
        // With total_power == 0 quorum can never be met
        assert_eq!(*tracker.get_status(&hash).unwrap(), FinalityStatus::Pending);
        assert!(!tracker.check_quorum(&hash));
    }
}
