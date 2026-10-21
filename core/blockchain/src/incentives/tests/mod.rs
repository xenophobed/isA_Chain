use crate::incentives::manager::IncentiveManager;
use crate::types::{Address, Hash};

mod program_tests;
mod reward_tests;

pub(crate) fn global_admin() -> Address {
    Address::from([0xAA; 20])
}

pub(crate) fn program_admin() -> Address {
    Address::from([0xBB; 20])
}

pub(crate) fn contributor(byte: u8) -> Address {
    Address::from([byte; 20])
}

pub(crate) fn rando() -> Address {
    Address::from([0xFF; 20])
}

pub(crate) fn default_manager() -> IncentiveManager {
    IncentiveManager::new(global_admin())
}

pub(crate) fn create_default_program(mgr: &mut IncentiveManager) -> Hash {
    mgr.create_program(
        "Bug Bounty Q1".to_string(),
        10_000,
        1_000,
        program_admin(),
        1,
    )
    .unwrap()
}
