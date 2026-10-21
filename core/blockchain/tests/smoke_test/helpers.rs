use isa_chain_core::{Address, Hash};

pub fn addr(byte: u8) -> Address {
    Address::from([byte; 20])
}

pub fn make_hash(seed: u8) -> Hash {
    Hash::new([seed; 32])
}
