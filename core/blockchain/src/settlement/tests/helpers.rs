use crate::settlement::engine::SettlementEngine;
use crate::types::constants::PROTOCOL_FEE_PERCENT;
use crate::types::{Address, Amount};

pub fn user() -> Address {
    Address::from([0x11; 20])
}

pub fn provider() -> Address {
    Address::from([0x22; 20])
}

pub fn provider2() -> Address {
    Address::from([0x33; 20])
}

pub fn engine() -> SettlementEngine {
    SettlementEngine::new(PROTOCOL_FEE_PERCENT)
}

pub const GROSS: Amount = 1_000_000;
