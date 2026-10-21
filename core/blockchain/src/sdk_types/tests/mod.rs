use crate::types::{Address, Hash};

pub(crate) fn zero_addr() -> Address {
    Address::new([0u8; 20])
}

pub(crate) fn zero_hash() -> Hash {
    Hash::ZERO
}

mod agent_version_tests;
mod wallet_channel_tests;
