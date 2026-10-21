use super::*;
use crate::types::{Address, Amount};

const ISA: Amount = 1_000_000_000_000_000_000;

pub(crate) fn isa(n: u128) -> Amount {
    n * ISA
}

pub(crate) fn addr(byte: u8) -> Address {
    Address::from([byte; 20])
}

pub(crate) fn system() -> VeISASystem {
    VeISASystem::default_system()
}

mod lock_tests;
mod voting_tests;
