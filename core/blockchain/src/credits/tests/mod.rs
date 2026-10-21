use super::*;
use crate::credits::{DEFAULT_CREDIT_PRICE_USD, DEFAULT_MIN_PURCHASE};
use crate::types::{Address, Amount};

pub(crate) fn admin() -> Address {
    Address::from([0xAA; 20])
}

pub(crate) fn user() -> Address {
    Address::from([0xBB; 20])
}

pub(crate) fn user2() -> Address {
    Address::from([0xCC; 20])
}

pub(crate) fn random_addr() -> Address {
    Address::from([0xDD; 20])
}

/// ISA price: $0.50 = 500_000 micro-USD
pub(crate) const ISA_PRICE: Amount = 500_000;

pub(crate) fn setup() -> CreditSystem {
    CreditSystem::new(DEFAULT_CREDIT_PRICE_USD, DEFAULT_MIN_PURCHASE, admin())
}

mod admin_tests;
mod purchase_tests;
