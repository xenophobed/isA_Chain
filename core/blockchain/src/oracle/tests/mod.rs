use super::*;
use crate::types::{Address, Amount};

pub(crate) fn admin() -> Address {
    Address::from([0xAA; 20])
}

pub(crate) fn feeder() -> Address {
    Address::from([0xBB; 20])
}

pub(crate) fn other_feeder() -> Address {
    Address::from([0xCC; 20])
}

pub(crate) fn random_addr() -> Address {
    Address::from([0xDD; 20])
}

/// $1.00 expressed in micro-USD
pub(crate) const ONE_DOLLAR: Amount = 1_000_000;

pub(crate) fn setup_oracle() -> PriceOracle {
    let mut oracle = PriceOracle::new(ONE_DOLLAR, admin(), 100, 1000);
    oracle.authorize_feeder(feeder(), &admin()).unwrap();
    oracle
}

mod conversion_tests;
mod submission_tests;
