use crate::balance_sync::engine::BalanceSyncEngine;
use crate::balance_sync::types::{DEFAULT_AUTO_FIX_THRESHOLD, DEFAULT_SYNC_INTERVAL_BLOCKS};
use crate::types::{Address, Amount};

pub fn addr(seed: u8) -> Address {
    Address::new([seed; 20])
}

/// 1 ISA = 1e18 wei; price = $1.00 (1_000_000 micro-USD).
/// 1 credit = $0.01 (10_000 micro-USD).
/// So 1 ISA → 100 credits.
pub const ISA_PRICE: Amount = 1_000_000; // $1.00
pub const CREDIT_PRICE: Amount = 10_000; // $0.01
pub const ONE_ISA: Amount = 1_000_000_000_000_000_000; // 1e18 wei

pub fn engine() -> BalanceSyncEngine {
    BalanceSyncEngine::new(DEFAULT_AUTO_FIX_THRESHOLD, DEFAULT_SYNC_INTERVAL_BLOCKS)
}
