use super::*;
use crate::types::constants::PROTOCOL_FEE_PERCENT;
use crate::types::{Address, Amount, BlockHeight, Hash};

pub(crate) fn sender() -> Address {
    Address::from([0xAA; 20])
}

pub(crate) fn receiver() -> Address {
    Address::from([0xBB; 20])
}

pub(crate) fn channel_id(seed: u8) -> Hash {
    Hash::hash_data(&[seed; 32])
}

/// Build a processor with default settings (max 100, 2.5% fee).
pub(crate) fn processor() -> BatchProcessor {
    BatchProcessor::new(100, PROTOCOL_FEE_PERCENT)
}

/// One valid settlement tuple.
pub(crate) fn settlement(
    seed: u8,
    amount: Amount,
    nonce: u64,
) -> (Hash, Address, Address, Amount, u64) {
    (channel_id(seed), sender(), receiver(), amount, nonce)
}

pub(crate) const HEIGHT: BlockHeight = 42;

mod creation_tests;
mod processing_tests;
