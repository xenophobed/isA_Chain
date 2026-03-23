use crate::subnet::SubnetId;
use crate::types::{Address, Amount, BlockHeight};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// EmissionSchedule
// ============================================================================

/// Controls the block-reward issuance schedule with halving support.
#[derive(Clone, Debug)]
pub struct EmissionSchedule {
    /// ISA (in wei) minted per block at genesis.
    pub base_reward_per_block: Amount,
    /// Number of blocks between each halving.
    pub halving_interval: u64,
    /// Minimum reward floor — reward never drops below this.
    pub min_reward: Amount,
    /// Running total of all ISA ever emitted.
    pub total_emitted: Amount,
    /// Block height at which the last emission occurred (0 = none yet).
    pub last_emission_height: BlockHeight,
}

// ============================================================================
// SubnetEmission
// ============================================================================

/// A record of one subnet's share of a block reward.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubnetEmission {
    pub subnet_id: SubnetId,
    pub amount: Amount,
    pub height: BlockHeight,
    /// Per-provider breakdown: `(address, share_amount)`.
    pub provider_shares: Vec<(Address, Amount)>,
}

// ============================================================================
// EmissionError
// ============================================================================

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EmissionError {
    #[error("No subnets provided")]
    NoSubnets,

    #[error("Invalid weights: weights must be non-zero and sum to 10 000")]
    InvalidWeights,

    #[error("Emission already processed at block {0}")]
    AlreadyEmitted(BlockHeight),

    #[error("Block reward is zero")]
    ZeroReward,
}

// ============================================================================
// EmissionController
// ============================================================================

pub struct EmissionController {
    pub schedule: EmissionSchedule,
    /// All individual subnet emission records across all blocks.
    pub emission_history: Vec<SubnetEmission>,
    /// Cumulative ISA emitted to each subnet since genesis.
    pub subnet_totals: HashMap<SubnetId, Amount>,
}

impl EmissionController {
    // ----------------------------------------------------------------
    // Constructor
    // ----------------------------------------------------------------

    pub fn new(base_reward: Amount, halving_interval: u64, min_reward: Amount) -> Self {
        EmissionController {
            schedule: EmissionSchedule {
                base_reward_per_block: base_reward,
                halving_interval,
                min_reward,
                total_emitted: 0,
                last_emission_height: 0,
            },
            emission_history: Vec::new(),
            subnet_totals: HashMap::new(),
        }
    }

    // ----------------------------------------------------------------
    // Reward calculation
    // ----------------------------------------------------------------

    /// Compute the block reward for a given height, applying halvings.
    ///
    /// reward = base_reward >> (height / halving_interval), clamped to min_reward.
    pub fn calculate_reward(&self, height: BlockHeight) -> Amount {
        let halvings = if self.schedule.halving_interval == 0 {
            0u32
        } else {
            // cap at 127 to avoid shifting a u128 into zero before the min clamp
            (height / self.schedule.halving_interval).min(127) as u32
        };

        let reward = self.schedule.base_reward_per_block >> halvings;
        reward.max(self.schedule.min_reward)
    }

    /// Alias for `calculate_reward` — returns the current rate without mutating.
    pub fn get_current_reward_rate(&self, height: BlockHeight) -> Amount {
        self.calculate_reward(height)
    }

    // ----------------------------------------------------------------
    // Emission
    // ----------------------------------------------------------------

    /// Distribute the block reward at `height` across subnets by weight,
    /// then within each subnet by provider-stake proportion.
    ///
    /// `subnet_weights`  — `SubnetId → emission weight (basis points, must sum to 10 000)`
    /// `provider_stakes` — `(SubnetId, Address) → stake amount`
    pub fn emit(
        &mut self,
        height: BlockHeight,
        subnet_weights: &HashMap<SubnetId, u32>,
        provider_stakes: &HashMap<(SubnetId, Address), Amount>,
    ) -> Result<Vec<SubnetEmission>, EmissionError> {
        // --- Guard: duplicate emission ----------------------------------------
        // We allow height 0 as first emission; track "already emitted" by
        // checking if any history entry exists for this height.
        if self.emission_history.iter().any(|e| e.height == height) {
            return Err(EmissionError::AlreadyEmitted(height));
        }

        // Also block re-emission at the same height tracked in the schedule
        // (catches the case where history is empty but last_emission_height matches
        //  because height == 0 was already processed).
        if height == self.schedule.last_emission_height && !self.emission_history.is_empty() {
            return Err(EmissionError::AlreadyEmitted(height));
        }

        // --- Guard: subnet weights --------------------------------------------
        if subnet_weights.is_empty() {
            return Err(EmissionError::NoSubnets);
        }

        let weight_sum: u32 = subnet_weights.values().sum();
        if weight_sum == 0 {
            return Err(EmissionError::InvalidWeights);
        }

        // --- Compute block reward ---------------------------------------------
        let block_reward = self.calculate_reward(height);
        if block_reward == 0 {
            return Err(EmissionError::ZeroReward);
        }

        // --- Distribute to subnets -------------------------------------------
        let mut emissions: Vec<SubnetEmission> = Vec::with_capacity(subnet_weights.len());

        for (&subnet_id, &weight) in subnet_weights {
            // subnet_amount = block_reward * weight / weight_sum
            // Use u128 arithmetic; weight_sum is at most 10 000 so no overflow risk.
            let subnet_amount: Amount =
                block_reward * weight as u128 / weight_sum as u128;

            // --- Distribute within subnet by stake proportion ----------------
            let provider_shares = Self::split_by_stake(subnet_id, subnet_amount, provider_stakes);

            // Update cumulative subnet total.
            *self.subnet_totals.entry(subnet_id).or_insert(0) += subnet_amount;

            emissions.push(SubnetEmission {
                subnet_id,
                amount: subnet_amount,
                height,
                provider_shares,
            });
        }

        // --- Update schedule bookkeeping -------------------------------------
        let total_distributed: Amount = emissions.iter().map(|e| e.amount).sum();
        self.schedule.total_emitted += total_distributed;
        self.schedule.last_emission_height = height;

        // Persist history.
        self.emission_history.extend(emissions.iter().cloned());

        Ok(emissions)
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    pub fn get_total_emitted(&self) -> Amount {
        self.schedule.total_emitted
    }

    pub fn get_subnet_total(&self, subnet_id: &SubnetId) -> Amount {
        self.subnet_totals.get(subnet_id).copied().unwrap_or(0)
    }

    /// Return all `SubnetEmission` records recorded at `height`.
    pub fn get_emission_at(&self, height: BlockHeight) -> Option<Vec<&SubnetEmission>> {
        let records: Vec<&SubnetEmission> = self
            .emission_history
            .iter()
            .filter(|e| e.height == height)
            .collect();
        if records.is_empty() {
            None
        } else {
            Some(records)
        }
    }

    // ----------------------------------------------------------------
    // Private helpers
    // ----------------------------------------------------------------

    /// Split `total` among providers in `subnet_id` proportional to their stake.
    /// If no providers are staked in the subnet, the entire amount goes unallocated
    /// (empty vec — the reward is still counted in subnet_totals for accounting).
    fn split_by_stake(
        subnet_id: SubnetId,
        total: Amount,
        provider_stakes: &HashMap<(SubnetId, Address), Amount>,
    ) -> Vec<(Address, Amount)> {
        // Collect (address, stake) for providers in this subnet.
        let stakers: Vec<(Address, Amount)> = provider_stakes
            .iter()
            .filter_map(|((sid, addr), &stake)| {
                if *sid == subnet_id && stake > 0 {
                    Some((*addr, stake))
                } else {
                    None
                }
            })
            .collect();

        if stakers.is_empty() {
            return Vec::new();
        }

        let total_stake: Amount = stakers.iter().map(|(_, s)| s).sum();
        if total_stake == 0 {
            return Vec::new();
        }

        stakers
            .iter()
            .map(|(addr, stake)| {
                // Compute share = total * stake / total_stake without overflow.
                // total, stake and total_stake are all u128 (wei amounts up to ~10^36).
                // Directly multiplying two such numbers overflows u128.
                // Solution: reduce the fraction by GCD first so the intermediate
                // product fits in u128.
                let g = gcd(total, total_stake);
                let reduced_total = total / g;
                let reduced_denom = total_stake / g;
                // reduced_total * stake may still overflow if both are large.
                // Apply a second reduction on (stake, reduced_denom).
                let g2 = gcd(*stake, reduced_denom);
                let reduced_stake = stake / g2;
                let reduced_denom2 = reduced_denom / g2;
                let share = reduced_total * reduced_stake / reduced_denom2;
                (*addr, share)
            })
            .collect()
    }
}

// ============================================================================
// Utilities
// ============================================================================

/// Euclidean GCD for u128.
fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    if a == 0 { 1 } else { a }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------------
    // Helpers
    // ----------------------------------------------------------------

    const ISA: Amount = 1_000_000_000_000_000_000; // 1 ISA in wei

    fn addr(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    /// Two-subnet weight map: Model=6000, Tools=4000 (sums to 10 000).
    fn two_subnet_weights() -> HashMap<SubnetId, u32> {
        let mut m = HashMap::new();
        m.insert(SubnetId::Model, 6000);
        m.insert(SubnetId::Tools, 4000);
        m
    }

    /// Controller with 100 ISA base reward, 1 000-block halving, 1 ISA min.
    fn controller() -> EmissionController {
        EmissionController::new(100 * ISA, 1_000, ISA)
    }

    fn empty_stakes() -> HashMap<(SubnetId, Address), Amount> {
        HashMap::new()
    }

    // ----------------------------------------------------------------
    // calculate_reward
    // ----------------------------------------------------------------

    #[test]
    fn test_calculate_reward() {
        let c = controller();
        // Block 0 — no halving yet.
        assert_eq!(c.calculate_reward(0), 100 * ISA);
        // Block 999 — still first epoch.
        assert_eq!(c.calculate_reward(999), 100 * ISA);
        // Block 1000 — first halving: 100 >> 1 = 50.
        assert_eq!(c.calculate_reward(1_000), 50 * ISA);
    }

    #[test]
    fn test_halving() {
        let c = controller();
        assert_eq!(c.calculate_reward(0), 100 * ISA);
        assert_eq!(c.calculate_reward(1_000), 50 * ISA);
        assert_eq!(c.calculate_reward(2_000), 25 * ISA);
        assert_eq!(c.calculate_reward(3_000), 12 * ISA + 500_000_000_000_000_000);
    }

    #[test]
    fn test_min_reward_floor() {
        // With a very large height the halved reward must not fall below min_reward.
        let c = controller(); // min = 1 ISA
        let reward = c.calculate_reward(1_000_000);
        assert!(reward >= ISA, "reward {reward} must be >= 1 ISA");
        assert_eq!(reward, ISA); // expected to hit the floor
    }

    // ----------------------------------------------------------------
    // emit — basic distribution
    // ----------------------------------------------------------------

    #[test]
    fn test_emit_distribution() {
        let mut c = controller();
        let weights = two_subnet_weights();
        let emissions = c.emit(1, &weights, &empty_stakes()).unwrap();

        assert_eq!(emissions.len(), 2);

        let total: Amount = emissions.iter().map(|e| e.amount).sum();
        // Total should equal block_reward (100 ISA).  Small rounding loss acceptable.
        assert!(total <= 100 * ISA);
        assert!(total >= 99 * ISA, "total {total} should be close to 100 ISA");
    }

    #[test]
    fn test_emit_proportional_by_weight() {
        let mut c = controller();
        let weights = two_subnet_weights(); // Model=6000, Tools=4000
        let emissions = c.emit(1, &weights, &empty_stakes()).unwrap();

        let model_amt = emissions.iter().find(|e| e.subnet_id == SubnetId::Model).unwrap().amount;
        let tools_amt = emissions.iter().find(|e| e.subnet_id == SubnetId::Tools).unwrap().amount;

        // Model should get 60 ISA, Tools 40 ISA.
        assert_eq!(model_amt, 60 * ISA);
        assert_eq!(tools_amt, 40 * ISA);
    }

    // ----------------------------------------------------------------
    // emit — per-provider stake split
    // ----------------------------------------------------------------

    #[test]
    fn test_emit_proportional_by_stake() {
        let mut c = controller();

        // Single subnet: Model = 10 000 weight (100%).
        let mut weights = HashMap::new();
        weights.insert(SubnetId::Model, 10_000u32);

        // Two providers: p1 stakes 3x, p2 stakes 1x → 75% / 25%.
        let p1 = addr(0x01);
        let p2 = addr(0x02);
        let mut stakes: HashMap<(SubnetId, Address), Amount> = HashMap::new();
        stakes.insert((SubnetId::Model, p1), 300 * ISA);
        stakes.insert((SubnetId::Model, p2), 100 * ISA);

        let emissions = c.emit(1, &weights, &stakes).unwrap();
        assert_eq!(emissions.len(), 1);

        let e = &emissions[0];
        assert_eq!(e.subnet_id, SubnetId::Model);

        let shares: HashMap<Address, Amount> = e.provider_shares.iter().cloned().collect();
        let s1 = *shares.get(&p1).unwrap();
        let s2 = *shares.get(&p2).unwrap();

        // s1 should be 75 ISA, s2 should be 25 ISA.
        assert_eq!(s1, 75 * ISA);
        assert_eq!(s2, 25 * ISA);
    }

    // ----------------------------------------------------------------
    // subnet_totals
    // ----------------------------------------------------------------

    #[test]
    fn test_subnet_totals() {
        let mut c = controller();
        let weights = two_subnet_weights(); // Model 60%, Tools 40%

        c.emit(1, &weights, &empty_stakes()).unwrap();
        c.emit(2, &weights, &empty_stakes()).unwrap();

        // After 2 blocks: Model = 2 * 60 = 120 ISA, Tools = 2 * 40 = 80 ISA.
        assert_eq!(c.get_subnet_total(&SubnetId::Model), 120 * ISA);
        assert_eq!(c.get_subnet_total(&SubnetId::Tools), 80 * ISA);
        assert_eq!(c.get_total_emitted(), 200 * ISA);
    }

    // ----------------------------------------------------------------
    // Error: AlreadyEmitted
    // ----------------------------------------------------------------

    #[test]
    fn test_already_emitted() {
        let mut c = controller();
        let weights = two_subnet_weights();

        c.emit(5, &weights, &empty_stakes()).unwrap();
        let err = c.emit(5, &weights, &empty_stakes()).unwrap_err();
        assert_eq!(err, EmissionError::AlreadyEmitted(5));
    }

    // ----------------------------------------------------------------
    // Error: NoSubnets
    // ----------------------------------------------------------------

    #[test]
    fn test_no_subnets_fails() {
        let mut c = controller();
        let err = c.emit(1, &HashMap::new(), &empty_stakes()).unwrap_err();
        assert_eq!(err, EmissionError::NoSubnets);
    }

    // ----------------------------------------------------------------
    // emission_history
    // ----------------------------------------------------------------

    #[test]
    fn test_emission_history() {
        let mut c = controller();
        let weights = two_subnet_weights();

        c.emit(10, &weights, &empty_stakes()).unwrap();

        let records = c.get_emission_at(10).unwrap();
        assert_eq!(records.len(), 2);
        assert!(records.iter().all(|e| e.height == 10));

        // Query for a block that was never emitted returns None.
        assert!(c.get_emission_at(99).is_none());
    }

    // ----------------------------------------------------------------
    // multiple consecutive emissions
    // ----------------------------------------------------------------

    #[test]
    fn test_multiple_emissions() {
        let mut c = controller();
        let weights = two_subnet_weights();

        for height in 1..=5u64 {
            let emissions = c.emit(height, &weights, &empty_stakes()).unwrap();
            assert_eq!(emissions.len(), 2);
        }

        // 5 blocks × 100 ISA each = 500 ISA total.
        assert_eq!(c.get_total_emitted(), 500 * ISA);
        // history should contain 5 × 2 = 10 records.
        assert_eq!(c.emission_history.len(), 10);
    }

    // ----------------------------------------------------------------
    // get_current_reward_rate
    // ----------------------------------------------------------------

    #[test]
    fn test_get_current_reward_rate() {
        let c = controller();
        assert_eq!(c.get_current_reward_rate(0), 100 * ISA);
        assert_eq!(c.get_current_reward_rate(1_000), 50 * ISA);
    }

    // ----------------------------------------------------------------
    // invalid weights (all zero)
    // ----------------------------------------------------------------

    #[test]
    fn test_invalid_weights_zero_sum() {
        let mut c = controller();
        let mut weights = HashMap::new();
        weights.insert(SubnetId::Model, 0u32);
        let err = c.emit(1, &weights, &empty_stakes()).unwrap_err();
        assert_eq!(err, EmissionError::InvalidWeights);
    }
}
