use crate::subnet::SubnetId;
use crate::types::{Address, Amount, BlockHeight};
use crate::types::constants::MAX_SLASH_PERCENT;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// SubnetUnbonding
// ============================================================================

/// A single pending unbonding chunk for a subnet stake.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubnetUnbonding {
    pub amount: Amount,
    pub completion_height: BlockHeight,
}

// ============================================================================
// SubnetStake
// ============================================================================

/// Per-(subnet, provider) stake record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubnetStake {
    pub provider: Address,
    pub subnet_id: SubnetId,
    pub amount: Amount,
    pub staked_at: BlockHeight,
    pub unbonding: Vec<SubnetUnbonding>,
}

// ============================================================================
// SubnetStakingError
// ============================================================================

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SubnetStakingError {
    #[error("Insufficient stake: required {required}, provided {provided}")]
    InsufficientStake { required: Amount, provided: Amount },

    #[error("No stake found for this provider/subnet combination")]
    NoStakeFound,

    #[error("Subnet not found")]
    SubnetNotFound,

    #[error("Insufficient balance for requested operation")]
    InsufficientBalance,

    #[error("Provider already has a stake in this subnet; use add_stake to increase")]
    AlreadyStaked,
}

// ============================================================================
// SubnetStakingManager
// ============================================================================

/// Manages per-subnet provider stakes with unbonding queues and slashing.
pub struct SubnetStakingManager {
    /// Primary stake store keyed by (subnet, provider).
    stakes: HashMap<(SubnetId, Address), SubnetStake>,
    /// Aggregate stake per subnet (active only, not unbonding).
    total_by_subnet: HashMap<SubnetId, Amount>,
    /// Which subnets each provider is staked in.
    provider_subnets: HashMap<Address, Vec<SubnetId>>,
    /// Number of blocks required before an unbonding entry is released.
    unbonding_period: u64,
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
            .or_insert_with(Vec::new)
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
        let slash_amount = (entry.amount as u128)
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
    pub fn get_stake(
        &self,
        provider: &Address,
        subnet_id: &SubnetId,
    ) -> Option<&SubnetStake> {
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const ISA: Amount = 1_000_000_000_000_000_000; // 1 ISA in wei

    fn isa(n: u128) -> Amount {
        n * ISA
    }

    fn provider(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    /// A fresh manager with a 100-block unbonding period.
    fn manager() -> SubnetStakingManager {
        SubnetStakingManager::new(100)
    }

    // ----------------------------------------------------------------

    #[test]
    fn test_stake_success() {
        let mut m = manager();
        let p = provider(0x01);
        m.stake(p, SubnetId::Model, isa(1_000), isa(1_000), 10)
            .unwrap();

        assert!(m.is_staked(&p, &SubnetId::Model));
        let s = m.get_stake(&p, &SubnetId::Model).unwrap();
        assert_eq!(s.amount, isa(1_000));
        assert_eq!(s.staked_at, 10);
        assert!(s.unbonding.is_empty());
        assert_eq!(m.get_subnet_total_stake(&SubnetId::Model), isa(1_000));
    }

    #[test]
    fn test_stake_below_minimum() {
        let mut m = manager();
        let p = provider(0x02);
        let err = m
            .stake(p, SubnetId::Tools, isa(499), isa(500), 1)
            .unwrap_err();
        assert_eq!(
            err,
            SubnetStakingError::InsufficientStake {
                required: isa(500),
                provided: isa(499),
            }
        );
        assert!(!m.is_staked(&p, &SubnetId::Tools));
    }

    #[test]
    fn test_add_stake() {
        let mut m = manager();
        let p = provider(0x03);
        m.stake(p, SubnetId::Storage, isa(500), isa(500), 1)
            .unwrap();

        m.add_stake(&p, &SubnetId::Storage, isa(200)).unwrap();

        let s = m.get_stake(&p, &SubnetId::Storage).unwrap();
        assert_eq!(s.amount, isa(700));
        assert_eq!(m.get_subnet_total_stake(&SubnetId::Storage), isa(700));
    }

    #[test]
    fn test_add_stake_no_stake_found() {
        let mut m = manager();
        let p = provider(0x04);
        let err = m.add_stake(&p, &SubnetId::Agent, isa(100)).unwrap_err();
        assert_eq!(err, SubnetStakingError::NoStakeFound);
    }

    #[test]
    fn test_begin_unstake() {
        let mut m = manager();
        let p = provider(0x05);
        m.stake(p, SubnetId::Compute, isa(2_000), isa(2_000), 1)
            .unwrap();

        m.begin_unstake(&p, &SubnetId::Compute, isa(500), 50)
            .unwrap();

        let s = m.get_stake(&p, &SubnetId::Compute).unwrap();
        assert_eq!(s.amount, isa(1_500));
        assert_eq!(s.unbonding.len(), 1);
        assert_eq!(s.unbonding[0].amount, isa(500));
        assert_eq!(s.unbonding[0].completion_height, 150); // 50 + 100
        assert_eq!(m.get_subnet_total_stake(&SubnetId::Compute), isa(1_500));
    }

    #[test]
    fn test_begin_unstake_insufficient_balance() {
        let mut m = manager();
        let p = provider(0x06);
        m.stake(p, SubnetId::Model, isa(1_000), isa(1_000), 1)
            .unwrap();

        let err = m
            .begin_unstake(&p, &SubnetId::Model, isa(1_001), 10)
            .unwrap_err();
        assert_eq!(err, SubnetStakingError::InsufficientBalance);
        // Stake unchanged
        assert_eq!(
            m.get_stake(&p, &SubnetId::Model).unwrap().amount,
            isa(1_000)
        );
    }

    #[test]
    fn test_complete_unbonding() {
        let mut m = manager();
        let p = provider(0x07);
        m.stake(p, SubnetId::Tools, isa(500), isa(500), 1).unwrap();
        m.begin_unstake(&p, &SubnetId::Tools, isa(200), 1).unwrap();

        // Before period ends — nothing released.
        let early = m.complete_unbonding(&p, &SubnetId::Tools, 50);
        assert_eq!(early, 0);
        assert_eq!(
            m.get_stake(&p, &SubnetId::Tools).unwrap().unbonding.len(),
            1
        );

        // At completion height — full amount released.
        let released = m.complete_unbonding(&p, &SubnetId::Tools, 101); // 1 + 100
        assert_eq!(released, isa(200));
        assert!(
            m.get_stake(&p, &SubnetId::Tools)
                .unwrap()
                .unbonding
                .is_empty()
        );
    }

    #[test]
    fn test_slash() {
        let mut m = manager();
        let p = provider(0x08);
        m.stake(p, SubnetId::Market, isa(1_000), isa(500), 1)
            .unwrap();

        // Slash 10% (1000 bps)
        let slashed = m.slash(&p, &SubnetId::Market, 1_000).unwrap();
        assert_eq!(slashed, isa(100));
        assert_eq!(
            m.get_stake(&p, &SubnetId::Market).unwrap().amount,
            isa(900)
        );
        assert_eq!(m.get_subnet_total_stake(&SubnetId::Market), isa(900));
    }

    #[test]
    fn test_slash_max_cap() {
        let mut m = manager();
        let p = provider(0x09);
        m.stake(p, SubnetId::Agent, isa(1_000), isa(1_000), 1)
            .unwrap();

        // Request 80% (8000 bps) — capped at 50% (5000 bps).
        let slashed = m.slash(&p, &SubnetId::Agent, 8_000).unwrap();
        assert_eq!(slashed, isa(500));
        assert_eq!(
            m.get_stake(&p, &SubnetId::Agent).unwrap().amount,
            isa(500)
        );
    }

    #[test]
    fn test_get_provider_total_stake() {
        let mut m = manager();
        let p = provider(0x0A);
        m.stake(p, SubnetId::Model, isa(1_000), isa(1_000), 1)
            .unwrap();
        m.stake(p, SubnetId::Tools, isa(500), isa(500), 1).unwrap();
        m.stake(p, SubnetId::Storage, isa(300), isa(300), 1)
            .unwrap();

        assert_eq!(m.get_provider_total_stake(&p), isa(1_800));
    }

    #[test]
    fn test_get_subnet_total_stake() {
        let mut m = manager();
        let p1 = provider(0x0B);
        let p2 = provider(0x0C);
        m.stake(p1, SubnetId::Compute, isa(2_000), isa(2_000), 1)
            .unwrap();
        m.stake(p2, SubnetId::Compute, isa(3_000), isa(2_000), 1)
            .unwrap();

        assert_eq!(m.get_subnet_total_stake(&SubnetId::Compute), isa(5_000));
    }

    #[test]
    fn test_provider_subnets() {
        let mut m = manager();
        let p = provider(0x0D);
        m.stake(p, SubnetId::Model, isa(1_000), isa(1_000), 1)
            .unwrap();
        m.stake(p, SubnetId::Agent, isa(1_000), isa(1_000), 1)
            .unwrap();

        let mut subnets = m.get_provider_subnets(&p);
        subnets.sort_by_key(|s| format!("{:?}", s));
        assert_eq!(subnets.len(), 2);
        assert!(subnets.contains(&SubnetId::Model));
        assert!(subnets.contains(&SubnetId::Agent));
    }

    #[test]
    fn test_no_stake_found() {
        let mut m = manager();
        let p = provider(0xFF);

        assert_eq!(
            m.add_stake(&p, &SubnetId::Tools, isa(100)),
            Err(SubnetStakingError::NoStakeFound)
        );
        assert_eq!(
            m.begin_unstake(&p, &SubnetId::Tools, isa(100), 1),
            Err(SubnetStakingError::NoStakeFound)
        );
        assert_eq!(
            m.slash(&p, &SubnetId::Tools, 1_000),
            Err(SubnetStakingError::NoStakeFound)
        );
        assert_eq!(m.complete_unbonding(&p, &SubnetId::Tools, 1000), 0);
    }

    #[test]
    fn test_already_staked() {
        let mut m = manager();
        let p = provider(0x0E);
        m.stake(p, SubnetId::Market, isa(500), isa(500), 1)
            .unwrap();

        let err = m
            .stake(p, SubnetId::Market, isa(500), isa(500), 2)
            .unwrap_err();
        assert_eq!(err, SubnetStakingError::AlreadyStaked);
    }
}
