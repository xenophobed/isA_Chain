use crate::types::{Address, Amount, BlockHeight, Timestamp};
use std::collections::HashSet;

use super::types::{OracleError, PricePoint, BPS_DIVISOR, MICRO_USD_SCALE};

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
    pub(crate) prices: Vec<PricePoint>,
    /// Current ISA price in micro-USD
    pub(crate) current_price: Amount,
    /// Addresses allowed to submit prices
    pub(crate) authorized_feeders: HashSet<Address>,
    /// Protocol admin (can authorize / revoke feeders)
    pub(crate) admin: Address,
    /// Maximum age in blocks before price is considered stale
    pub(crate) staleness_threshold: u64,
    /// Maximum allowed price change per update in basis points
    pub(crate) max_deviation_bps: u32,
    /// Height of the last price update (None if no price submitted yet)
    pub(crate) last_update_height: Option<BlockHeight>,
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
