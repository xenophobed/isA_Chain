use crate::subnet_staking::manager::SubnetStakingManager;
use crate::types::{Address, Amount};

pub const ISA: Amount = 1_000_000_000_000_000_000; // 1 ISA in wei

pub fn isa(n: u128) -> Amount {
    n * ISA
}

pub fn provider(byte: u8) -> Address {
    Address::from([byte; 20])
}

/// A fresh manager with a 100-block unbonding period.
pub fn manager() -> SubnetStakingManager {
    SubnetStakingManager::new(100)
}
