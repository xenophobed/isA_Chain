use crate::subnet::SubnetId;
use crate::types::{Address, Amount, BlockHeight};
use serde::{Deserialize, Serialize};

/// Controls the block-reward issuance schedule with halving support.
#[derive(Clone, Debug)]
pub struct EmissionSchedule {
    /// ISA (in wei) minted per block at genesis.
    pub base_reward_per_block: Amount,
    /// Number of blocks between each halving.
    pub halving_interval: u64,
    /// Minimum reward floor — reward never drops below this.
    pub min_reward: Amount,
    /// Running total of all ISA ever emitted.
    pub total_emitted: Amount,
    /// Block height at which the last emission occurred (0 = none yet).
    pub last_emission_height: BlockHeight,
}

/// A record of one subnet's share of a block reward.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubnetEmission {
    pub subnet_id: SubnetId,
    pub amount: Amount,
    pub height: BlockHeight,
    /// Per-provider breakdown: `(address, share_amount)`.
    pub provider_shares: Vec<(Address, Amount)>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EmissionError {
    #[error("No subnets provided")]
    NoSubnets,

    #[error("Invalid weights: weights must be non-zero and sum to 10 000")]
    InvalidWeights,

    #[error("Emission already processed at block {0}")]
    AlreadyEmitted(BlockHeight),

    #[error("Block reward is zero")]
    ZeroReward,
}
