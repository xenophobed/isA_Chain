use crate::types::{Address, Amount};
use crate::withdrawal::manager::WithdrawalManager;
use crate::withdrawal::types::{
    DEFAULT_COOLDOWN_BLOCKS, DEFAULT_DAILY_LIMIT, DEFAULT_MIN_WITHDRAWAL,
    DEFAULT_WITHDRAWAL_FEE_BPS,
};

mod calculation_tests;
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

/// ISA price: $0.50 = 500_000 micro-USD
pub(crate) const ISA_PRICE: Amount = 500_000;

pub(crate) fn setup() -> WithdrawalManager {
    WithdrawalManager::new(
        DEFAULT_WITHDRAWAL_FEE_BPS, // 50 bps
        DEFAULT_COOLDOWN_BLOCKS,    // 100 blocks
        DEFAULT_MIN_WITHDRAWAL,     // 1_000 credits
        DEFAULT_DAILY_LIMIT,        // 100_000_000 credits
        crate::credits::DEFAULT_CREDIT_PRICE_USD,
    )
}
