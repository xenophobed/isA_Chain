use crate::types::{Address, Amount, BlockHeight, Timestamp};
use std::collections::HashSet;

/// Default number of blocks before a price is considered stale
pub const DEFAULT_STALENESS_THRESHOLD: u64 = 100;

/// Default maximum price deviation per update in basis points (1000 = 10%)
pub const DEFAULT_MAX_DEVIATION_BPS: u32 = 1000;

/// Micro-USD scale factor: 1_000_000 micro-USD = $1.00
pub const MICRO_USD_SCALE: Amount = 1_000_000;

/// Basis points divisor
const BPS_DIVISOR: u128 = 10_000;

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

// ====================================================================
// PriceOracle
// ====================================================================

/// On-chain price oracle providing ISA/USD price feeds for credit conversion.
///
/// Maintains a history of [`PricePoint`] observations submitted by authorized
/// feeders.  Prices are expressed in micro-USD (1_000_000 = $1.00).  The
/// oracle rejects updates that deviate too far from the current price and
/// marks the price as stale when the last update is too old (in blocks).
pub struct PriceOracle {
    /// Full price history (append-only)
    prices: Vec<PricePoint>,
    /// Current ISA price in micro-USD
    current_price: Amount,
    /// Addresses allowed to submit prices
    authorized_feeders: HashSet<Address>,
    /// Protocol admin (can authorize / revoke feeders)
    admin: Address,
    /// Maximum age in blocks before price is considered stale
    staleness_threshold: u64,
    /// Maximum allowed price change per update in basis points
    max_deviation_bps: u32,
    /// Height of the last price update (None if no price submitted yet)
    last_update_height: Option<BlockHeight>,
}

impl PriceOracle {
    // ----------------------------------------------------------------
    // Construction
    // ----------------------------------------------------------------

    /// Create a new `PriceOracle` with an initial price.
    ///
    /// `initial_price` is in micro-USD; pass `0` to indicate no price is set
    /// yet (subsequent calls to [`get_price`](PriceOracle::get_price) will
    /// return [`OracleError::NoPrice`] until the first submission).
    pub fn new(
        initial_price: Amount,
        admin: Address,
        staleness_threshold: u64,
        max_deviation_bps: u32,
    ) -> Self {
        PriceOracle {
            prices: Vec::new(),
            current_price: initial_price,
            authorized_feeders: HashSet::new(),
            admin,
            staleness_threshold,
            max_deviation_bps,
            last_update_height: None,
        }
    }

    // ----------------------------------------------------------------
    // Price submission
    // ----------------------------------------------------------------

    /// Submit a new price observation.
    ///
    /// Returns `Ok(())` on success.  Fails with:
    /// - [`OracleError::UnauthorizedFeeder`] — caller is not in the feeder set
    /// - [`OracleError::InvalidPrice`] — `price` is zero
    /// - [`OracleError::PriceDeviationTooHigh`] — deviation from current price
    ///   exceeds `max_deviation_bps` (only checked once a current price exists)
    pub fn submit_price(
        &mut self,
        price: Amount,
        height: BlockHeight,
        timestamp: Timestamp,
        feeder: &Address,
    ) -> Result<(), OracleError> {
        if !self.authorized_feeders.contains(feeder) {
            return Err(OracleError::UnauthorizedFeeder(*feeder));
        }

        if price == 0 {
            return Err(OracleError::InvalidPrice);
        }

        // Deviation check — only when there is an existing price
        if self.current_price > 0 {
            self.check_deviation(price)?;
        }

        let point = PricePoint {
            price,
            timestamp,
            height,
            feeder: *feeder,
        };

        self.prices.push(point);
        self.current_price = price;
        self.last_update_height = Some(height);

        Ok(())
    }

    // ----------------------------------------------------------------
    // Price queries
    // ----------------------------------------------------------------

    /// Get the current price in micro-USD.
    ///
    /// Returns [`OracleError::NoPrice`] if no price has ever been submitted.
    /// Note: does **not** check staleness — call [`is_stale`](PriceOracle::is_stale)
    /// separately when staleness should block usage.
    pub fn get_price(&self) -> Result<Amount, OracleError> {
        if self.prices.is_empty() {
            return Err(OracleError::NoPrice);
        }
        Ok(self.current_price)
    }

    /// Look up the most recent [`PricePoint`] submitted at or before `height`.
    pub fn get_price_at(&self, height: BlockHeight) -> Option<&PricePoint> {
        // prices are stored in submission order; find the last one at or before height
        self.prices.iter().rev().find(|p| p.height <= height)
    }

    /// Returns `true` if the last price update is older than `staleness_threshold` blocks.
    ///
    /// Also returns `true` when no price has been submitted yet.
    pub fn is_stale(&self, current_height: BlockHeight) -> bool {
        match self.last_update_height {
            None => true,
            Some(last) => current_height.saturating_sub(last) > self.staleness_threshold,
        }
    }

    // ----------------------------------------------------------------
    // Feeder authorization
    // ----------------------------------------------------------------

    /// Authorize an address to submit prices.  Only the admin may call this.
    pub fn authorize_feeder(
        &mut self,
        address: Address,
        admin: &Address,
    ) -> Result<(), OracleError> {
        if !self.is_admin(admin) {
            return Err(OracleError::UnauthorizedAdmin(*admin));
        }
        self.authorized_feeders.insert(address);
        Ok(())
    }

    /// Revoke a feeder's authorization.  Only the admin may call this.
    pub fn revoke_feeder(
        &mut self,
        address: &Address,
        admin: &Address,
    ) -> Result<(), OracleError> {
        if !self.is_admin(admin) {
            return Err(OracleError::UnauthorizedAdmin(*admin));
        }
        self.authorized_feeders.remove(address);
        Ok(())
    }

    // ----------------------------------------------------------------
    // Conversion helpers
    // ----------------------------------------------------------------

    /// Convert an ISA amount to micro-USD using the current price.
    ///
    /// Both amounts are in their respective micro units.  The formula is:
    /// `usd_amount = isa_amount * current_price / MICRO_USD_SCALE`
    ///
    /// Returns [`OracleError::NoPrice`] when no price exists and
    /// [`OracleError::PriceStale`] is **not** checked here — callers that
    /// want staleness enforcement should call [`is_stale`](PriceOracle::is_stale)
    /// first.
    pub fn convert_isa_to_usd(&self, isa_amount: Amount) -> Result<Amount, OracleError> {
        let price = self.get_price()?;
        // isa_amount * price / MICRO_USD_SCALE
        // Use u128 arithmetic; amounts are already micro-ISA, price is micro-USD per ISA
        let usd = isa_amount
            .checked_mul(price)
            .map(|v| v / MICRO_USD_SCALE)
            .unwrap_or(u128::MAX); // saturate on overflow rather than panic
        Ok(usd)
    }

    /// Convert a micro-USD amount to ISA using the current price.
    ///
    /// The formula is: `isa_amount = usd_amount * MICRO_USD_SCALE / current_price`
    ///
    /// Returns [`OracleError::NoPrice`] when no price exists.
    pub fn convert_usd_to_isa(&self, usd_amount: Amount) -> Result<Amount, OracleError> {
        let price = self.get_price()?;
        if price == 0 {
            return Err(OracleError::InvalidPrice);
        }
        let isa = usd_amount
            .checked_mul(MICRO_USD_SCALE)
            .map(|v| v / price)
            .unwrap_or(u128::MAX);
        Ok(isa)
    }

    /// Compute the time-weighted average price from the last `num_points` observations.
    ///
    /// Uses a simple arithmetic mean of the last N price points as a
    /// conservative TWAP approximation.  Returns [`OracleError::NoPrice`]
    /// when fewer than one point is available.
    pub fn get_twap(&self, num_points: usize) -> Result<Amount, OracleError> {
        if self.prices.is_empty() {
            return Err(OracleError::NoPrice);
        }

        let window: Vec<&PricePoint> = self
            .prices
            .iter()
            .rev()
            .take(num_points)
            .collect();

        let count = window.len() as u128;
        let sum: u128 = window.iter().map(|p| p.price).sum();
        Ok(sum / count)
    }

    // ----------------------------------------------------------------
    // Private helpers
    // ----------------------------------------------------------------

    fn is_admin(&self, address: &Address) -> bool {
        *address == self.admin
    }

    /// Check that `new_price` does not deviate from `current_price` by more
    /// than `max_deviation_bps` basis points.
    fn check_deviation(&self, new_price: Amount) -> Result<(), OracleError> {
        let current = self.current_price;
        if current == 0 {
            return Ok(());
        }

        // Calculate absolute difference
        let diff = new_price.abs_diff(current);

        // deviation_bps = diff * 10_000 / current
        let deviation_bps = diff
            .checked_mul(BPS_DIVISOR)
            .map(|v| v / current)
            .unwrap_or(u128::MAX);

        if deviation_bps > self.max_deviation_bps as u128 {
            Err(OracleError::PriceDeviationTooHigh)
        } else {
            Ok(())
        }
    }
}

// ====================================================================
// Tests
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------------
    // Test fixtures
    // ----------------------------------------------------------------

    fn admin() -> Address {
        Address::from([0xAA; 20])
    }

    fn feeder() -> Address {
        Address::from([0xBB; 20])
    }

    fn other_feeder() -> Address {
        Address::from([0xCC; 20])
    }

    fn random_addr() -> Address {
        Address::from([0xDD; 20])
    }

    /// $1.00 expressed in micro-USD
    const ONE_DOLLAR: Amount = 1_000_000;

    fn setup_oracle() -> PriceOracle {
        let mut oracle = PriceOracle::new(ONE_DOLLAR, admin(), 100, 1000);
        oracle.authorize_feeder(feeder(), &admin()).unwrap();
        oracle
    }

    // ----------------------------------------------------------------
    // test_submit_price
    // ----------------------------------------------------------------

    #[test]
    fn test_submit_price() {
        let mut oracle = setup_oracle();

        // First submission: sets price and records a point
        oracle
            .submit_price(ONE_DOLLAR, 10, 1_000_000, &feeder())
            .unwrap();

        assert_eq!(oracle.get_price().unwrap(), ONE_DOLLAR);
        assert_eq!(oracle.prices.len(), 1);
        assert_eq!(oracle.last_update_height, Some(10));
    }

    // ----------------------------------------------------------------
    // test_unauthorized_feeder_fails
    // ----------------------------------------------------------------

    #[test]
    fn test_unauthorized_feeder_fails() {
        let mut oracle = setup_oracle();

        let result = oracle.submit_price(ONE_DOLLAR, 10, 1_000_000, &random_addr());
        assert!(matches!(result, Err(OracleError::UnauthorizedFeeder(_))));
    }

    // ----------------------------------------------------------------
    // test_price_deviation_check
    // ----------------------------------------------------------------

    #[test]
    fn test_price_deviation_check() {
        let mut oracle = setup_oracle();

        // Set an initial price
        oracle
            .submit_price(ONE_DOLLAR, 10, 1_000_000, &feeder())
            .unwrap();

        // A 5% move — within the 10% limit
        let within_limit = ONE_DOLLAR + ONE_DOLLAR / 20; // +5%
        assert!(oracle
            .submit_price(within_limit, 11, 1_000_001, &feeder())
            .is_ok());

        // A 15% move — exceeds the 10% limit
        let above_limit = within_limit + within_limit * 15 / 100;
        let result = oracle.submit_price(above_limit, 12, 1_000_002, &feeder());
        assert_eq!(result, Err(OracleError::PriceDeviationTooHigh));
    }

    // ----------------------------------------------------------------
    // test_price_staleness
    // ----------------------------------------------------------------

    #[test]
    fn test_price_staleness() {
        let mut oracle = setup_oracle();

        // Submit at height 10
        oracle
            .submit_price(ONE_DOLLAR, 10, 1_000_000, &feeder())
            .unwrap();

        // At height 109 (99 blocks later) — not stale
        assert!(!oracle.is_stale(109));

        // At height 111 (101 blocks later) — stale
        assert!(oracle.is_stale(111));
    }

    // ----------------------------------------------------------------
    // test_convert_isa_to_usd
    // ----------------------------------------------------------------

    #[test]
    fn test_convert_isa_to_usd() {
        let mut oracle = setup_oracle();
        oracle
            .submit_price(ONE_DOLLAR, 1, 1_000_000, &feeder())
            .unwrap();

        // 1 ISA (1_000_000 micro-ISA) at $1.00/ISA → $1.00 (1_000_000 micro-USD)
        let result = oracle.convert_isa_to_usd(1_000_000).unwrap();
        assert_eq!(result, 1_000_000);

        // 2_000_000 micro-ISA at $1.00 → 2_000_000 micro-USD
        let result2 = oracle.convert_isa_to_usd(2_000_000).unwrap();
        assert_eq!(result2, 2_000_000);
    }

    // ----------------------------------------------------------------
    // test_convert_usd_to_isa
    // ----------------------------------------------------------------

    #[test]
    fn test_convert_usd_to_isa() {
        let mut oracle = setup_oracle();
        oracle
            .submit_price(ONE_DOLLAR, 1, 1_000_000, &feeder())
            .unwrap();

        // $1.00 (1_000_000 micro-USD) at $1.00/ISA → 1_000_000 micro-ISA
        let result = oracle.convert_usd_to_isa(1_000_000).unwrap();
        assert_eq!(result, 1_000_000);

        // $2.00 → 2_000_000 micro-ISA
        let result2 = oracle.convert_usd_to_isa(2_000_000).unwrap();
        assert_eq!(result2, 2_000_000);
    }

    // ----------------------------------------------------------------
    // test_authorize_revoke_feeder
    // ----------------------------------------------------------------

    #[test]
    fn test_authorize_revoke_feeder() {
        let mut oracle = PriceOracle::new(ONE_DOLLAR, admin(), 100, 1000);
        let addr = other_feeder();

        // Not yet authorized
        assert!(!oracle.authorized_feeders.contains(&addr));

        // Authorize
        oracle.authorize_feeder(addr, &admin()).unwrap();
        assert!(oracle.authorized_feeders.contains(&addr));

        // Revoke
        oracle.revoke_feeder(&addr, &admin()).unwrap();
        assert!(!oracle.authorized_feeders.contains(&addr));

        // Non-admin cannot authorize
        let result = oracle.authorize_feeder(addr, &random_addr());
        assert!(matches!(result, Err(OracleError::UnauthorizedAdmin(_))));

        // Non-admin cannot revoke
        oracle.authorize_feeder(addr, &admin()).unwrap();
        let result2 = oracle.revoke_feeder(&addr, &random_addr());
        assert!(matches!(result2, Err(OracleError::UnauthorizedAdmin(_))));
    }

    // ----------------------------------------------------------------
    // test_twap_calculation
    // ----------------------------------------------------------------

    #[test]
    fn test_twap_calculation() {
        let mut oracle = setup_oracle();

        // Submit 3 price points: 1_000_000, 1_050_000, 1_100_000
        oracle
            .submit_price(1_000_000, 10, 10_000, &feeder())
            .unwrap();
        oracle
            .submit_price(1_050_000, 11, 11_000, &feeder())
            .unwrap();
        oracle
            .submit_price(1_100_000, 12, 12_000, &feeder())
            .unwrap();

        // TWAP of last 3 points = (1_000_000 + 1_050_000 + 1_100_000) / 3 = 1_050_000
        let twap = oracle.get_twap(3).unwrap();
        assert_eq!(twap, 1_050_000);

        // TWAP of last 2 points = (1_050_000 + 1_100_000) / 2 = 1_075_000
        let twap2 = oracle.get_twap(2).unwrap();
        assert_eq!(twap2, 1_075_000);
    }

    // ----------------------------------------------------------------
    // test_invalid_price_fails
    // ----------------------------------------------------------------

    #[test]
    fn test_invalid_price_fails() {
        let mut oracle = setup_oracle();

        let result = oracle.submit_price(0, 10, 1_000_000, &feeder());
        assert_eq!(result, Err(OracleError::InvalidPrice));
    }

    // ----------------------------------------------------------------
    // test_get_price_at_height
    // ----------------------------------------------------------------

    #[test]
    fn test_get_price_at_height() {
        let mut oracle = setup_oracle();

        oracle
            .submit_price(1_000_000, 10, 10_000, &feeder())
            .unwrap();
        oracle
            .submit_price(1_050_000, 20, 20_000, &feeder())
            .unwrap();

        // Exact height match
        let point = oracle.get_price_at(10).unwrap();
        assert_eq!(point.price, 1_000_000);
        assert_eq!(point.height, 10);

        // Height between two points — should return the earlier one
        let point2 = oracle.get_price_at(15).unwrap();
        assert_eq!(point2.price, 1_000_000);

        // Height at or after the latest point
        let point3 = oracle.get_price_at(20).unwrap();
        assert_eq!(point3.price, 1_050_000);

        // Height before any submission
        assert!(oracle.get_price_at(5).is_none());
    }

    // ----------------------------------------------------------------
    // test_no_price_fails
    // ----------------------------------------------------------------

    #[test]
    fn test_no_price_fails() {
        let oracle = PriceOracle::new(ONE_DOLLAR, admin(), 100, 1000);

        // No submissions yet — get_price should fail
        assert_eq!(oracle.get_price(), Err(OracleError::NoPrice));

        // Conversion should also fail
        assert_eq!(oracle.convert_isa_to_usd(1_000_000), Err(OracleError::NoPrice));
        assert_eq!(oracle.convert_usd_to_isa(1_000_000), Err(OracleError::NoPrice));

        // TWAP should fail
        assert_eq!(oracle.get_twap(5), Err(OracleError::NoPrice));

        // is_stale should return true with no submissions
        assert!(oracle.is_stale(0));
    }
}
