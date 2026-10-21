use super::*;
use crate::types::{Address, Hash};

pub(crate) fn make_address(seed: u8) -> Address {
    Address::new([seed; 20])
}

pub(crate) fn make_hash(seed: u8) -> Hash {
    Hash::new([seed; 32])
}

pub(crate) fn default_system() -> ReputationSystem {
    ReputationSystem::new(make_address(0x01), 5000)
}

mod query_tests;
mod recording_tests;
