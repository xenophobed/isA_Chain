use crate::settlement_fallback::manager::FallbackManager;
use crate::settlement_fallback::types::{
    DEFAULT_AUTO_CREDIT_THRESHOLD, DEFAULT_MAX_DEFER_BLOCKS,
};
use crate::types::{Address, Amount, Hash};

mod action_tests;
mod lifecycle_tests;

// ---- Fixtures ----------------------------------------------------------

pub(crate) fn settlement_id() -> Hash {
    Hash::hash_data(b"settlement_1")
}

pub(crate) fn user() -> Address {
    Address::from([0x11; 20])
}

pub(crate) fn provider() -> Address {
    Address::from([0x22; 20])
}

pub(crate) fn manager() -> FallbackManager {
    FallbackManager::new(DEFAULT_MAX_DEFER_BLOCKS, DEFAULT_AUTO_CREDIT_THRESHOLD)
}

pub(crate) const REQUIRED: Amount = 5_000_000;
