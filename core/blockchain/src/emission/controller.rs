use super::types::*;
use crate::subnet::SubnetId;
use crate::types::{Address, Amount, BlockHeight};
use std::collections::HashMap;

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

/// Euclidean GCD for u128.
fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    if a == 0 {
        1
    } else {
        a
    }
}
