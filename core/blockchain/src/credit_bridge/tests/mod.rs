use crate::credit_bridge::bridge::CreditBridge;
use crate::types::{Address, Amount};

mod initiation_tests;
mod lifecycle_tests;

// ----------------------------------------------------------------
// Fixtures
// ----------------------------------------------------------------

pub(crate) fn user() -> Address {
    Address::from([0xAA; 20])
}

pub(crate) fn user2() -> Address {
    Address::from([0xBB; 20])
}

pub(crate) fn unknown_user() -> Address {
    Address::from([0xFF; 20])
}

/// 2 ISA in micro-ISA
pub(crate) const TWO_ISA: Amount = 2_000_000;

/// 100 credits
pub(crate) const HUNDRED_CREDITS: Amount = 100;

pub(crate) fn setup() -> CreditBridge {
    CreditBridge::new()
}
