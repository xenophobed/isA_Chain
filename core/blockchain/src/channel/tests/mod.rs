use crate::channel::manager::ChannelManager;
use crate::types::{Address, Amount, BlockHeight, Hash};

mod dispute_tests;
mod lifecycle_tests;

// -----------------------------------------------------------------------
// Test helpers
// -----------------------------------------------------------------------

pub(crate) fn sender() -> Address {
    Address::from([0xAA; 20])
}

pub(crate) fn receiver() -> Address {
    Address::from([0xBB; 20])
}

pub(crate) fn third_party() -> Address {
    Address::from([0xCC; 20])
}

pub(crate) const DEPOSIT: Amount = 1_000;
pub(crate) const OPEN_HEIGHT: BlockHeight = 100;
pub(crate) const DISPUTE_PERIOD: u64 = 50;
pub(crate) const DEFAULT_EXPIRY: u64 = 10_000;

pub(crate) fn manager() -> ChannelManager {
    ChannelManager::new(DISPUTE_PERIOD, DEFAULT_EXPIRY)
}

pub(crate) fn open_channel(mgr: &mut ChannelManager) -> Hash {
    mgr.open_channel(sender(), receiver(), DEPOSIT, OPEN_HEIGHT)
        .expect("open_channel failed")
}
