use crate::pos::engine::PoSConsensus;
use crate::types::Address;

pub fn addr(byte: u8) -> Address {
    Address::from([byte; 20])
}

/// Default engine: epoch=100, min_validators=1.
pub fn engine() -> PoSConsensus {
    PoSConsensus::new(100, 1)
}
