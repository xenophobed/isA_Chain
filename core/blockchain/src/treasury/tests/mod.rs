use super::*;
use crate::types::constants::PROTOCOL_FEE_PERCENT;
use crate::types::Address;

pub(crate) fn admin() -> Address {
    Address::from([0xAA; 20])
}

pub(crate) fn other() -> Address {
    Address::from([0xBB; 20])
}

pub(crate) fn staker(byte: u8) -> Address {
    Address::from([byte; 20])
}

pub(crate) fn default_treasury() -> ProtocolTreasury {
    ProtocolTreasury::new(PROTOCOL_FEE_PERCENT, admin())
}

mod distribute_tests;
mod fee_tests;
