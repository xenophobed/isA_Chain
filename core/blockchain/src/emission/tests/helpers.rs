use crate::emission::controller::EmissionController;
use crate::subnet::SubnetId;
use crate::types::{Address, Amount};
use std::collections::HashMap;

pub const ISA: Amount = 1_000_000_000_000_000_000; // 1 ISA in wei

pub fn addr(byte: u8) -> Address {
    Address::from([byte; 20])
}

/// Two-subnet weight map: Model=6000, Tools=4000 (sums to 10 000).
pub fn two_subnet_weights() -> HashMap<SubnetId, u32> {
    let mut m = HashMap::new();
    m.insert(SubnetId::Model, 6000);
    m.insert(SubnetId::Tools, 4000);
    m
}

/// Controller with 100 ISA base reward, 1 000-block halving, 1 ISA min.
pub fn controller() -> EmissionController {
    EmissionController::new(100 * ISA, 1_000, ISA)
}

pub fn empty_stakes() -> HashMap<(SubnetId, Address), Amount> {
    HashMap::new()
}
