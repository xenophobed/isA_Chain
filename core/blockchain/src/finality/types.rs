use crate::types::{Address, Amount, BlockHeight, Hash, Signature};
use serde::{Deserialize, Serialize};

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
