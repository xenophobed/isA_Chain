use super::primitives::*;

pub const MAIN_CHAIN_ID: ChainId = 15489;
pub const TEST_CHAIN_ID: ChainId = 15490;

pub const BLOCK_TIME_MS: u64 = 3000; // 3 seconds
pub const MAX_BLOCK_SIZE: usize = 1024 * 1024; // 1MB
pub const MAX_GAS_PER_BLOCK: Gas = 30_000_000;
pub const BASE_GAS_PRICE: GasPrice = 1_000_000_000; // 1 Gwei

pub const GENESIS_TIMESTAMP: Timestamp = 1704067200000; // Jan 1, 2024 00:00:00 UTC
pub const INITIAL_SUPPLY: Amount = 1_000_000_000_000_000_000_000_000_000; // 1B ISA tokens

pub const VALIDATOR_MIN_STAKE: Amount = 32_000_000_000_000_000_000_000; // 32,000 ISA
pub const DELEGATION_MIN_AMOUNT: Amount = 1_000_000_000_000_000_000; // 1 ISA

// Compute marketplace constants
pub const PROVIDER_MIN_STAKE: Amount = 1_000_000_000_000_000_000_000; // 1,000 ISA
pub const JOB_MIN_ESCROW: Amount = 100_000_000_000_000_000; // 0.1 ISA
pub const DISPUTE_WINDOW_SECS: u64 = 86400; // 24 hours
pub const SETTLEMENT_DELAY_SECS: u64 = 3600; // 1 hour grace period
pub const MAX_SLASH_PERCENT: u32 = 5000; // 50% max slash
pub const PROTOCOL_FEE_PERCENT: u32 = 250; // 2.5% protocol fee
