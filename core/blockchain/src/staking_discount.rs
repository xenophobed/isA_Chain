//! Staking Discount — Reputation-Based Stake Reduction
//!
//! Providers with high reputation scores earn discounts on their required stake,
//! incentivizing quality service and long-term participation in the network.

use crate::types::Amount;
use serde::{Deserialize, Serialize};

// ============================================================================
// Constants
// ============================================================================

const DEFAULT_MAX_DISCOUNT_BPS: u32 = 5_000; // 50%
const BASIS_POINTS: u128 = 10_000;

// ============================================================================
// Errors
// ============================================================================

/// Errors for the staking discount system
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StakingDiscountError {
    #[error("Invalid tier configuration: overlapping or misconfigured tiers")]
    InvalidTier,

    #[error("Provider does not qualify for any discount tier")]
    NoDiscount,
}

// ============================================================================
// DiscountTier
// ============================================================================

/// A named discount tier granted to providers meeting a minimum reputation score
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscountTier {
    /// Minimum reputation score required (0–10 000 basis points)
    pub min_reputation: u32,
    /// Stake reduction in basis points (e.g. 1000 = 10% discount)
    pub discount_bps: u32,
    /// Human-readable name for this tier
    pub name: String,
}

// ============================================================================
// StakingDiscountManager
// ============================================================================

/// Manages reputation-based staking discounts for compute providers
#[derive(Clone, Debug)]
pub struct StakingDiscountManager {
    /// Discount tiers sorted by `min_reputation` ascending
    pub tiers: Vec<DiscountTier>,
    /// Maximum discount that can ever be applied (in basis points)
    pub max_discount_bps: u32,
}

impl StakingDiscountManager {
    // -------------------------------------------------------------------------
    // Construction
    // -------------------------------------------------------------------------

    /// Create an empty manager with no tiers and the given max-discount cap.
    pub fn new(max_discount: u32) -> Self {
        Self {
            tiers: Vec::new(),
            max_discount_bps: max_discount,
        }
    }

    /// Create a manager pre-loaded with the four default tiers:
    /// Bronze (6000 rep, 10%), Silver (7500 rep, 20%),
    /// Gold (8500 rep, 30%), Diamond (9500 rep, 50%).
    pub fn with_default_tiers() -> Self {
        let mut manager = Self::new(DEFAULT_MAX_DISCOUNT_BPS);

        let defaults = vec![
            DiscountTier {
                min_reputation: 6_000,
                discount_bps: 1_000,
                name: "Bronze".to_string(),
            },
            DiscountTier {
                min_reputation: 7_500,
                discount_bps: 2_000,
                name: "Silver".to_string(),
            },
            DiscountTier {
                min_reputation: 8_500,
                discount_bps: 3_000,
                name: "Gold".to_string(),
            },
            DiscountTier {
                min_reputation: 9_500,
                discount_bps: 5_000,
                name: "Diamond".to_string(),
            },
        ];

        // These are valid by construction — unwrap is safe here.
        for tier in defaults {
            manager.add_tier(tier).expect("default tiers are always valid");
        }

        manager
    }

    // -------------------------------------------------------------------------
    // Tier management
    // -------------------------------------------------------------------------

    /// Add a custom discount tier.
    ///
    /// Returns `InvalidTier` if another tier already uses the same
    /// `min_reputation` threshold (which would be ambiguous).
    pub fn add_tier(&mut self, tier: DiscountTier) -> Result<(), StakingDiscountError> {
        // Reject duplicate thresholds — two tiers at the same floor are ambiguous.
        if self
            .tiers
            .iter()
            .any(|t| t.min_reputation == tier.min_reputation)
        {
            return Err(StakingDiscountError::InvalidTier);
        }

        self.tiers.push(tier);
        // Keep sorted ascending by min_reputation so get_tier is correct.
        self.tiers.sort_by_key(|t| t.min_reputation);
        Ok(())
    }

    /// Return a reference to all tiers (sorted ascending by min_reputation).
    pub fn get_tiers(&self) -> &[DiscountTier] {
        &self.tiers
    }

    // -------------------------------------------------------------------------
    // Queries
    // -------------------------------------------------------------------------

    /// Return the highest tier a provider qualifies for given their reputation,
    /// or `None` if they don't meet any tier's threshold.
    pub fn get_tier(&self, reputation: u32) -> Option<&DiscountTier> {
        // Tiers are sorted ascending; iterate in reverse to find the highest
        // threshold that is still ≤ reputation.
        self.tiers
            .iter()
            .rev()
            .find(|t| reputation >= t.min_reputation)
    }

    /// Return the discount in basis points for the given reputation score.
    /// Returns 0 if the provider doesn't qualify for any tier.
    /// The result is capped at `self.max_discount_bps`.
    pub fn get_discount(&self, reputation: u32) -> u32 {
        let raw = self
            .get_tier(reputation)
            .map(|t| t.discount_bps)
            .unwrap_or(0);

        raw.min(self.max_discount_bps)
    }

    /// Calculate the stake actually required after applying the reputation discount.
    ///
    /// Formula: `base_stake * (10_000 - discount_bps) / 10_000`
    pub fn calculate_required_stake(&self, base_stake: Amount, reputation: u32) -> Amount {
        let discount = self.get_discount(reputation) as u128;
        base_stake * (BASIS_POINTS - discount) / BASIS_POINTS
    }

    /// Calculate how much the provider saves compared to the base stake.
    ///
    /// Formula: `base_stake - calculate_required_stake(base_stake, reputation)`
    pub fn get_savings(&self, base_stake: Amount, reputation: u32) -> Amount {
        let required = self.calculate_required_stake(base_stake, reputation);
        base_stake.saturating_sub(required)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    fn manager() -> StakingDiscountManager {
        StakingDiscountManager::with_default_tiers()
    }

    const BASE_STAKE: Amount = 1_000_000_000_000_000_000_000; // 1 000 ISA in wei

    // -------------------------------------------------------------------------
    // Tier configuration
    // -------------------------------------------------------------------------

    #[test]
    fn test_default_tiers() {
        let mgr = manager();
        let tiers = mgr.get_tiers();

        assert_eq!(tiers.len(), 4);

        // Verify order and contents
        assert_eq!(tiers[0].name, "Bronze");
        assert_eq!(tiers[0].min_reputation, 6_000);
        assert_eq!(tiers[0].discount_bps, 1_000);

        assert_eq!(tiers[1].name, "Silver");
        assert_eq!(tiers[1].min_reputation, 7_500);
        assert_eq!(tiers[1].discount_bps, 2_000);

        assert_eq!(tiers[2].name, "Gold");
        assert_eq!(tiers[2].min_reputation, 8_500);
        assert_eq!(tiers[2].discount_bps, 3_000);

        assert_eq!(tiers[3].name, "Diamond");
        assert_eq!(tiers[3].min_reputation, 9_500);
        assert_eq!(tiers[3].discount_bps, 5_000);
    }

    // -------------------------------------------------------------------------
    // Discount lookup
    // -------------------------------------------------------------------------

    #[test]
    fn test_no_discount_low_reputation() {
        let mgr = manager();
        // Below 6 000 → no tier qualifies
        assert_eq!(mgr.get_discount(0), 0);
        assert_eq!(mgr.get_discount(5_000), 0);
        assert_eq!(mgr.get_discount(5_999), 0);
        assert!(mgr.get_tier(5_999).is_none());
    }

    #[test]
    fn test_bronze_discount() {
        let mgr = manager();
        // Exactly at the Bronze threshold
        assert_eq!(mgr.get_discount(6_000), 1_000);
        assert_eq!(mgr.get_tier(6_000).unwrap().name, "Bronze");

        // Somewhere between Bronze and Silver
        assert_eq!(mgr.get_discount(7_000), 1_000);
        assert_eq!(mgr.get_tier(7_000).unwrap().name, "Bronze");
    }

    #[test]
    fn test_silver_discount() {
        let mgr = manager();
        assert_eq!(mgr.get_discount(7_500), 2_000);
        assert_eq!(mgr.get_tier(7_500).unwrap().name, "Silver");

        // Between Silver and Gold
        assert_eq!(mgr.get_discount(8_000), 2_000);
        assert_eq!(mgr.get_tier(8_000).unwrap().name, "Silver");
    }

    #[test]
    fn test_gold_discount() {
        let mgr = manager();
        assert_eq!(mgr.get_discount(8_500), 3_000);
        assert_eq!(mgr.get_tier(8_500).unwrap().name, "Gold");

        // Between Gold and Diamond
        assert_eq!(mgr.get_discount(9_000), 3_000);
        assert_eq!(mgr.get_tier(9_000).unwrap().name, "Gold");
    }

    #[test]
    fn test_diamond_discount() {
        let mgr = manager();
        assert_eq!(mgr.get_discount(9_500), 5_000);
        assert_eq!(mgr.get_tier(9_500).unwrap().name, "Diamond");

        // Perfect score
        assert_eq!(mgr.get_discount(10_000), 5_000);
        assert_eq!(mgr.get_tier(10_000).unwrap().name, "Diamond");
    }

    // -------------------------------------------------------------------------
    // Stake calculations
    // -------------------------------------------------------------------------

    #[test]
    fn test_calculate_required_stake() {
        let mgr = manager();

        // No discount — full stake required
        let full = mgr.calculate_required_stake(BASE_STAKE, 5_000);
        assert_eq!(full, BASE_STAKE);

        // Bronze: 10% off → 90% of base
        let bronze = mgr.calculate_required_stake(BASE_STAKE, 6_000);
        assert_eq!(bronze, BASE_STAKE * 9_000 / 10_000);

        // Silver: 20% off → 80% of base
        let silver = mgr.calculate_required_stake(BASE_STAKE, 7_500);
        assert_eq!(silver, BASE_STAKE * 8_000 / 10_000);

        // Gold: 30% off → 70% of base
        let gold = mgr.calculate_required_stake(BASE_STAKE, 8_500);
        assert_eq!(gold, BASE_STAKE * 7_000 / 10_000);

        // Diamond: 50% off → 50% of base
        let diamond = mgr.calculate_required_stake(BASE_STAKE, 9_500);
        assert_eq!(diamond, BASE_STAKE / 2);
    }

    #[test]
    fn test_get_savings() {
        let mgr = manager();

        // No tier → zero savings
        assert_eq!(mgr.get_savings(BASE_STAKE, 5_000), 0);

        // Bronze: saves 10%
        let saved_bronze = mgr.get_savings(BASE_STAKE, 6_000);
        assert_eq!(saved_bronze, BASE_STAKE / 10);

        // Diamond: saves 50%
        let saved_diamond = mgr.get_savings(BASE_STAKE, 9_500);
        assert_eq!(saved_diamond, BASE_STAKE / 2);
    }

    // -------------------------------------------------------------------------
    // Custom tiers
    // -------------------------------------------------------------------------

    #[test]
    fn test_add_custom_tier() {
        let mut mgr = StakingDiscountManager::new(DEFAULT_MAX_DISCOUNT_BPS);

        let tier = DiscountTier {
            min_reputation: 5_000,
            discount_bps: 500,
            name: "Starter".to_string(),
        };

        mgr.add_tier(tier).unwrap();
        assert_eq!(mgr.get_tiers().len(), 1);
        assert_eq!(mgr.get_discount(5_000), 500);
        assert_eq!(mgr.get_discount(4_999), 0);
    }

    #[test]
    fn test_add_duplicate_tier_is_error() {
        let mut mgr = manager();

        // Duplicate min_reputation threshold
        let duplicate = DiscountTier {
            min_reputation: 6_000, // same as Bronze
            discount_bps: 800,
            name: "BronzeAlt".to_string(),
        };

        assert_eq!(
            mgr.add_tier(duplicate).unwrap_err(),
            StakingDiscountError::InvalidTier
        );
    }

    // -------------------------------------------------------------------------
    // Max discount cap
    // -------------------------------------------------------------------------

    #[test]
    fn test_max_discount_cap() {
        // Cap at 20% even though Diamond would normally give 50%
        let mut mgr = StakingDiscountManager::new(2_000);

        let tier = DiscountTier {
            min_reputation: 9_500,
            discount_bps: 5_000, // would be 50% without cap
            name: "Diamond".to_string(),
        };
        mgr.add_tier(tier).unwrap();

        // get_discount respects the cap
        assert_eq!(mgr.get_discount(9_500), 2_000);

        // Required stake uses the capped discount
        let required = mgr.calculate_required_stake(BASE_STAKE, 9_500);
        assert_eq!(required, BASE_STAKE * 8_000 / 10_000); // 80% (20% off)
    }

    // -------------------------------------------------------------------------
    // Boundary conditions
    // -------------------------------------------------------------------------

    #[test]
    fn test_boundary_reputation() {
        let mgr = manager();

        // One below each threshold → still previous tier
        assert_eq!(mgr.get_discount(7_499), 1_000); // just below Silver → Bronze
        assert_eq!(mgr.get_discount(8_499), 2_000); // just below Gold → Silver
        assert_eq!(mgr.get_discount(9_499), 3_000); // just below Diamond → Gold

        // Exactly at each threshold
        assert_eq!(mgr.get_discount(7_500), 2_000); // Silver
        assert_eq!(mgr.get_discount(8_500), 3_000); // Gold
        assert_eq!(mgr.get_discount(9_500), 5_000); // Diamond
    }
}
