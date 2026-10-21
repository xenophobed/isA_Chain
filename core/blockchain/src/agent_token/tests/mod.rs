use crate::agent_token::*;
use crate::types::{Address, Hash};

pub fn make_address(seed: u8) -> Address {
    Address::new([seed; 20])
}

pub fn make_hash(seed: u8) -> Hash {
    Hash::new([seed; 32])
}

pub fn linear_curve() -> BondingCurve {
    BondingCurve::Linear { slope: SCALE }
}

pub fn quad_curve() -> BondingCurve {
    BondingCurve::Quadratic { coefficient: SCALE }
}

mod advanced_tests;
mod linear_tests;
