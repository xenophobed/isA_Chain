use super::*;
use crate::deposit::DEPOSIT_DEFAULT_CREDIT_PRICE_USD;
use crate::types::{Address, Amount};

pub(crate) fn user() -> Address {
    Address::from([0xAA; 20])
}

pub(crate) fn user2() -> Address {
    Address::from([0xBB; 20])
}

/// ISA price: $0.50 = 500_000 micro-USD
pub(crate) const ISA_PRICE: Amount = 500_000;

/// 2 ISA in micro-ISA (smallest unit)
pub(crate) const TWO_ISA: Amount = 2_000_000;

pub(crate) fn setup() -> DepositManager {
    DepositManager::new(DEPOSIT_DEFAULT_CREDIT_PRICE_USD)
}

mod calculation_tests;
mod lifecycle_tests;
