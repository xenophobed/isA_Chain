use super::types::*;
use crate::subnet::SubnetId;
use crate::types::constants::MAX_SLASH_PERCENT;
use crate::types::{Address, Amount, BlockHeight};
use std::collections::HashMap;

/// Manages per-subnet provider stakes with unbonding queues and slashing.
pub struct SubnetStakingManager {
    /// Primary stake store keyed by (subnet, provider).
    pub(crate) stakes: HashMap<(SubnetId, Address), SubnetStake>,
    /// Aggregate stake per subnet (active only, not unbonding).
    pub(crate) total_by_subnet: HashMap<SubnetId, Amount>,
    /// Which subnets each provider is staked in.
    pub(crate) provider_subnets: HashMap<Address, Vec<SubnetId>>,
    /// Number of blocks required before an unbonding entry is released.
    pub(crate) unbonding_period: u64,
}

impl SubnetStakingManager {
    // ----------------------------------------------------------------
    // Constructor
    // ----------------------------------------------------------------

    /// Create a new manager.  `unbonding_period` is the number of blocks
    /// a provider must wait after calling `begin_unstake` before funds are
    /// released by `complete_unbonding`.  Defaults to 100 in tests.
    pub fn new(unbonding_period: u64) -> Self {
        SubnetStakingManager {
            stakes: HashMap::new(),
            total_by_subnet: HashMap::new(),
            provider_subnets: HashMap::new(),
            unbonding_period,
        }
    }

    // ----------------------------------------------------------------
    // Staking
    // ----------------------------------------------------------------

    /// Initial stake of `amount` ISA by `provider` into `subnet_id`.
    ///
    /// Fails if:
    /// - `amount` < `min_stake`
    /// - `provider` already has an active stake entry in this subnet
    pub fn stake(
        &mut self,
        provider: Address,
        subnet_id: SubnetId,
        amount: Amount,
        min_stake: Amount,
        height: BlockHeight,
    ) -> Result<(), SubnetStakingError> {
        if amount < min_stake {
            return Err(SubnetStakingError::InsufficientStake {
                required: min_stake,
                provided: amount,
            });
        }

        if self.stakes.contains_key(&(subnet_id, provider)) {
            return Err(SubnetStakingError::AlreadyStaked);
        }

        self.stakes.insert(
            (subnet_id, provider),
            SubnetStake {
                provider,
                subnet_id,
                amount,
                staked_at: height,
                unbonding: Vec::new(),
            },
        );

        *self.total_by_subnet.entry(subnet_id).or_insert(0) += amount;

        self.provider_subnets
            .entry(provider)
            .or_default()
            .push(subnet_id);

        Ok(())
    }

    /// Add `amount` to an **existing** stake entry.
    ///
    /// Fails if the provider has no existing stake in this subnet.
    pub fn add_stake(
        &mut self,
        provider: &Address,
        subnet_id: &SubnetId,
        amount: Amount,
    ) -> Result<(), SubnetStakingError> {
        let entry = self
            .stakes
            .get_mut(&(*subnet_id, *provider))
            .ok_or(SubnetStakingError::NoStakeFound)?;

        entry.amount += amount;
        *self.total_by_subnet.entry(*subnet_id).or_insert(0) += amount;

        Ok(())
    }

    // ----------------------------------------------------------------
    // Unbonding
    // ----------------------------------------------------------------

    /// Begin unbonding `amount` from the provider's stake in `subnet_id`.
    ///
    /// The amount is deducted from the active stake immediately and queued
    /// for release after `unbonding_period` blocks.
    ///
    /// Fails if:
    /// - no stake found
    /// - `amount` > active staked amount
    pub fn begin_unstake(
        &mut self,
        provider: &Address,
        subnet_id: &SubnetId,
        amount: Amount,
        height: BlockHeight,
    ) -> Result<(), SubnetStakingError> {
        let entry = self
            .stakes
            .get_mut(&(*subnet_id, *provider))
            .ok_or(SubnetStakingError::NoStakeFound)?;

        if amount > entry.amount {
            return Err(SubnetStakingError::InsufficientBalance);
        }

        entry.amount -= amount;

        // Decrement subnet aggregate (active stake only).
        if let Some(total) = self.total_by_subnet.get_mut(subnet_id) {
            *total = total.saturating_sub(amount);
        }

        entry.unbonding.push(SubnetUnbonding {
            amount,
            completion_height: height + self.unbonding_period,
        });

        Ok(())
    }

    /// Release all unbonding chunks whose `completion_height` <= `current_height`.
    ///
    /// Returns the total amount unlocked (caller is responsible for crediting
    /// the provider's balance).
    pub fn complete_unbonding(
        &mut self,
        provider: &Address,
        subnet_id: &SubnetId,
        current_height: BlockHeight,
    ) -> Amount {
        let entry = match self.stakes.get_mut(&(*subnet_id, *provider)) {
            Some(e) => e,
            None => return 0,
        };

        let mut released: Amount = 0;
        entry.unbonding.retain(|u| {
            if u.completion_height <= current_height {
                released += u.amount;
                false
            } else {
                true
            }
        });

        released
    }

    // ----------------------------------------------------------------
    // Slashing
    // ----------------------------------------------------------------

    /// Slash `percent_bps` basis points (1 bps = 0.01 %) from the provider's
    /// active stake in `subnet_id`.
    ///
    /// * Maximum allowed slash: `MAX_SLASH_PERCENT` (5 000 bps / 50 %).
    /// * The slashed amount is removed from both the stake entry and the
    ///   subnet aggregate, then returned so the caller can redirect it
    ///   (e.g., burn or redistribute).
    ///
    /// Fails if no stake entry exists.
    pub fn slash(
        &mut self,
        provider: &Address,
        subnet_id: &SubnetId,
        percent_bps: u32,
    ) -> Result<Amount, SubnetStakingError> {
        let entry = self
            .stakes
            .get_mut(&(*subnet_id, *provider))
            .ok_or(SubnetStakingError::NoStakeFound)?;

        let effective_bps = percent_bps.min(MAX_SLASH_PERCENT);
        let slash_amount = entry
            .amount
            .saturating_mul(effective_bps as u128)
            / 10_000;

        entry.amount -= slash_amount;

        if let Some(total) = self.total_by_subnet.get_mut(subnet_id) {
            *total = total.saturating_sub(slash_amount);
        }

        Ok(slash_amount)
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    /// Look up the stake record for a (provider, subnet) pair.
    pub fn get_stake(&self, provider: &Address, subnet_id: &SubnetId) -> Option<&SubnetStake> {
        self.stakes.get(&(*subnet_id, *provider))
    }

    /// Sum of active (non-unbonding) stakes for `provider` across all subnets.
    pub fn get_provider_total_stake(&self, provider: &Address) -> Amount {
        self.stakes
            .iter()
            .filter_map(|((_, addr), entry)| {
                if addr == provider {
                    Some(entry.amount)
                } else {
                    None
                }
            })
            .sum()
    }

    /// Sum of active (non-unbonding) stakes for all providers in `subnet_id`.
    pub fn get_subnet_total_stake(&self, subnet_id: &SubnetId) -> Amount {
        self.total_by_subnet.get(subnet_id).copied().unwrap_or(0)
    }

    /// Return the list of subnet IDs the provider is staked in.
    pub fn get_provider_subnets(&self, provider: &Address) -> Vec<SubnetId> {
        self.provider_subnets
            .get(provider)
            .cloned()
            .unwrap_or_default()
    }

    /// Returns `true` if the provider has an active stake entry in `subnet_id`.
    pub fn is_staked(&self, provider: &Address, subnet_id: &SubnetId) -> bool {
        self.stakes.contains_key(&(*subnet_id, *provider))
    }
}
