use crate::agent_wallet::*;
use crate::types::{Address, Amount, Hash};

// 1 ISA in wei
pub const ONE_ISA: Amount = 1_000_000_000_000_000_000;
pub const DEFAULT_PERIOD: u64 = 1000;

pub fn make_factory() -> AgentWalletFactory {
    AgentWalletFactory::new(ONE_ISA, DEFAULT_PERIOD)
}

pub fn make_agent_id(seed: u8) -> Hash {
    Hash::new([seed; 32])
}

pub fn make_owner(seed: u8) -> Address {
    Address::from([seed; 20])
}

mod creation_and_funding_tests;
mod spending_and_limits_tests;
