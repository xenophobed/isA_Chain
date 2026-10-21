use crate::types::{Address, Amount, BlockHeight, Timestamp};

/// Default number of blocks before a price is considered stale
pub const DEFAULT_STALENESS_THRESHOLD: u64 = 100;

/// Default maximum price deviation per update in basis points (1000 = 10%)
pub const DEFAULT_MAX_DEVIATION_BPS: u32 = 1000;

/// Micro-USD scale factor: 1_000_000 micro-USD = $1.00
pub const MICRO_USD_SCALE: Amount = 1_000_000;

/// Basis points divisor
pub(crate) const BPS_DIVISOR: u128 = 10_000;

// ====================================================================
// Errors
// ====================================================================

/// Errors related to oracle operations
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OracleError {
    #[error("Unauthorized feeder: {0} is not an authorized price feeder")]
    UnauthorizedFeeder(Address),

    #[error("Unauthorized admin operation: {0} is not the admin")]
    UnauthorizedAdmin(Address),

    #[error("Price is stale: current price exceeds staleness threshold")]
    PriceStale,

    #[error("Price deviation too high: update exceeds maximum allowed deviation")]
    PriceDeviationTooHigh,

    #[error("Invalid price: price must be greater than zero")]
    InvalidPrice,

    #[error("No price: no price has been submitted yet")]
    NoPrice,
}

// ====================================================================
// PricePoint
// ====================================================================

/// A single price observation submitted by an authorized feeder
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PricePoint {
    /// Price in micro-USD (1_000_000 = $1.00)
    pub price: Amount,
    /// Unix timestamp in milliseconds when the price was submitted
    pub timestamp: Timestamp,
    /// Block height at submission
    pub height: BlockHeight,
    /// Address of the feeder who submitted this price
    pub feeder: Address,
}
