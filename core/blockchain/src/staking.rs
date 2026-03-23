use crate::types::{Address, Amount, BlockHeight};
use crate::types::constants::VALIDATOR_MIN_STAKE;
use std::collections::HashMap;

/// Errors related to staking operations
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StakingError {
    #[error("Insufficient stake: minimum required is {min}, provided {provided}")]
    InsufficientStake { min: Amount, provided: Amount },

    #[error("Insufficient balance: cannot unstake {requested}, only {available} staked")]
    InsufficientBalance { requested: Amount, available: Amount },

    #[error("No stake found for address")]
    NoStakeFound,

    #[error("Stake already exists for address; use add_stake to increase")]
    StakeAlreadyExists,

    #[error("Unbonding in progress; wait for unbonding to complete before withdrawing")]
    UnbondingInProgress,
}

/// A single pending unbonding chunk
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnbondingEntry {
    /// Amount being unbonded
    pub amount: Amount,
    /// Block height at which this chunk is fully unlocked
    pub completion_height: BlockHeight,
}

/// Per-address stake record
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StakeEntry {
    /// Currently staked (does not include amounts in unbonding)
    pub amount: Amount,
    /// Block height when the initial stake was created
    pub staked_at: BlockHeight,
    /// Pending unlock chunks
    pub unbonding: Vec<UnbondingEntry>,
}

/// StakingVault — holds ISA collateral for providers and validators.
///
/// Tracks per-address stake, manages the unbonding queue, and provides
/// slashing support.  All amounts are in the base ISA unit (wei-equivalent).
pub struct StakingVault {
    /// Per-address stake records
    stakes: HashMap<Address, StakeEntry>,
    /// Sum of all active (non-unbonding) stakes
    total_staked: Amount,
    /// Minimum initial stake (default: VALIDATOR_MIN_STAKE)
    min_stake: Amount,
    /// Number of blocks that must pass before unbonded funds are released
    unbonding_period: u64,
}

impl StakingVault {
    // ----------------------------------------------------------------
    // Constructor
    // ----------------------------------------------------------------

    /// Create a new vault.
    ///
    /// * `min_stake`        — minimum ISA required for an initial `stake()` call.
    /// * `unbonding_period` — blocks that must elapse before `complete_unbonding`
    ///   releases funds.
    pub fn new(min_stake: Amount, unbonding_period: u64) -> Self {
        StakingVault {
            stakes: HashMap::new(),
            total_staked: 0,
            min_stake,
            unbonding_period,
        }
    }

    /// Convenience constructor using the protocol default minimum stake
    /// (`VALIDATOR_MIN_STAKE = 32 000 ISA`) and a 100-block unbonding period.
    pub fn default_vault() -> Self {
        Self::new(VALIDATOR_MIN_STAKE, 100)
    }

    // ----------------------------------------------------------------
    // Staking
    // ----------------------------------------------------------------

    /// Lock `amount` ISA for `address` as the **initial** stake.
    ///
    /// Fails if:
    /// - `amount` < `self.min_stake`
    /// - `address` already has a stake entry (use `add_stake` instead)
    pub fn stake(
        &mut self,
        address: Address,
        amount: Amount,
        height: BlockHeight,
    ) -> Result<(), StakingError> {
        if amount < self.min_stake {
            return Err(StakingError::InsufficientStake {
                min: self.min_stake,
                provided: amount,
            });
        }
        if self.stakes.contains_key(&address) {
            return Err(StakingError::StakeAlreadyExists);
        }

        self.stakes.insert(
            address,
            StakeEntry {
                amount,
                staked_at: height,
                unbonding: Vec::new(),
            },
        );
        self.total_staked += amount;
        Ok(())
    }

    /// Add `amount` to an **existing** stake entry.
    ///
    /// Fails if the address has no existing stake.
    pub fn add_stake(
        &mut self,
        address: &Address,
        amount: Amount,
    ) -> Result<(), StakingError> {
        let entry = self
            .stakes
            .get_mut(address)
            .ok_or(StakingError::NoStakeFound)?;

        entry.amount += amount;
        self.total_staked += amount;
        Ok(())
    }

    // ----------------------------------------------------------------
    // Unbonding
    // ----------------------------------------------------------------

    /// Begin unbonding `amount` from `address`'s stake.
    ///
    /// The amount is deducted from the active stake immediately and queued
    /// for release after `unbonding_period` blocks.
    ///
    /// Fails if:
    /// - address has no stake
    /// - `amount` > active staked amount
    pub fn begin_unstake(
        &mut self,
        address: &Address,
        amount: Amount,
        current_height: BlockHeight,
    ) -> Result<(), StakingError> {
        let entry = self
            .stakes
            .get_mut(address)
            .ok_or(StakingError::NoStakeFound)?;

        if amount > entry.amount {
            return Err(StakingError::InsufficientBalance {
                requested: amount,
                available: entry.amount,
            });
        }

        entry.amount -= amount;
        self.total_staked -= amount;

        entry.unbonding.push(UnbondingEntry {
            amount,
            completion_height: current_height + self.unbonding_period,
        });

        Ok(())
    }

    /// Release all unbonding chunks whose `completion_height` <= `current_height`.
    ///
    /// Returns the total amount unlocked (caller is responsible for crediting
    /// the account balance).
    pub fn complete_unbonding(
        &mut self,
        address: &Address,
        current_height: BlockHeight,
    ) -> Amount {
        let entry = match self.stakes.get_mut(address) {
            Some(e) => e,
            None => return 0,
        };

        let mut released: Amount = 0;
        entry.unbonding.retain(|u| {
            if u.completion_height <= current_height {
                released += u.amount;
                false // remove from queue
            } else {
                true // keep
            }
        });

        released
    }

    // ----------------------------------------------------------------
    // Slashing
    // ----------------------------------------------------------------

    /// Slash `percent_bps` basis points (1 bps = 0.01 %) from `address`'s
    /// active stake.
    ///
    /// * Maximum allowed slash: 5 000 bps (50 %).
    /// * The slashed amount is removed from `total_staked` and returned so
    ///   the caller can redirect it (e.g., burn or redistribute).
    ///
    /// Fails if the address has no stake.
    pub fn slash(
        &mut self,
        address: &Address,
        percent_bps: u32,
    ) -> Result<Amount, StakingError> {
        use crate::types::constants::MAX_SLASH_PERCENT;

        let entry = self
            .stakes
            .get_mut(address)
            .ok_or(StakingError::NoStakeFound)?;

        // Cap at protocol maximum
        let effective_bps = percent_bps.min(MAX_SLASH_PERCENT);

        // slash_amount = stake * bps / 10_000  (integer, rounds down)
        let slash_amount = entry.amount
            .saturating_mul(effective_bps as u128)
            / 10_000;

        entry.amount -= slash_amount;
        self.total_staked -= slash_amount;

        Ok(slash_amount)
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    /// Look up the stake entry for `address`.
    pub fn get_stake(&self, address: &Address) -> Option<&StakeEntry> {
        self.stakes.get(address)
    }

    /// Sum of all active (non-unbonding) stakes.
    pub fn get_total_staked(&self) -> Amount {
        self.total_staked
    }

    /// Returns `true` if `address` has an active stake entry.
    pub fn is_staked(&self, address: &Address) -> bool {
        self.stakes.contains_key(address)
    }
}

// ====================================================================
// Tests
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::constants::VALIDATOR_MIN_STAKE;

    // ---- helpers ---------------------------------------------------

    fn addr(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    /// Vault with 32 000 ISA minimum and 100-block unbonding.
    fn vault() -> StakingVault {
        StakingVault::default_vault()
    }

    // ----------------------------------------------------------------

    #[test]
    fn test_stake_success() {
        let mut v = vault();
        let a = addr(0x01);
        assert!(v.stake(a, VALIDATOR_MIN_STAKE, 1).is_ok());
        assert!(v.is_staked(&a));
        assert_eq!(v.get_total_staked(), VALIDATOR_MIN_STAKE);

        let entry = v.get_stake(&a).unwrap();
        assert_eq!(entry.amount, VALIDATOR_MIN_STAKE);
        assert_eq!(entry.staked_at, 1);
        assert!(entry.unbonding.is_empty());
    }

    #[test]
    fn test_stake_below_minimum_fails() {
        let mut v = vault();
        let a = addr(0x02);
        let below_min = VALIDATOR_MIN_STAKE - 1;
        let result = v.stake(a, below_min, 1);
        assert_eq!(
            result,
            Err(StakingError::InsufficientStake {
                min: VALIDATOR_MIN_STAKE,
                provided: below_min,
            })
        );
        assert!(!v.is_staked(&a));
    }

    #[test]
    fn test_add_stake() {
        let mut v = vault();
        let a = addr(0x03);
        v.stake(a, VALIDATOR_MIN_STAKE, 10).unwrap();

        let extra = 5_000_000_000_000_000_000_000_u128; // 5 000 ISA
        v.add_stake(&a, extra).unwrap();

        let entry = v.get_stake(&a).unwrap();
        assert_eq!(entry.amount, VALIDATOR_MIN_STAKE + extra);
        assert_eq!(v.get_total_staked(), VALIDATOR_MIN_STAKE + extra);
    }

    #[test]
    fn test_begin_unstake() {
        let mut v = vault();
        let a = addr(0x04);
        v.stake(a, VALIDATOR_MIN_STAKE, 1).unwrap();

        let unstake_amount = VALIDATOR_MIN_STAKE / 4;
        v.begin_unstake(&a, unstake_amount, 50).unwrap();

        let entry = v.get_stake(&a).unwrap();
        assert_eq!(entry.amount, VALIDATOR_MIN_STAKE - unstake_amount);
        assert_eq!(v.get_total_staked(), VALIDATOR_MIN_STAKE - unstake_amount);
        assert_eq!(entry.unbonding.len(), 1);
        assert_eq!(entry.unbonding[0].amount, unstake_amount);
        assert_eq!(entry.unbonding[0].completion_height, 150); // 50 + 100
    }

    #[test]
    fn test_complete_unbonding() {
        let mut v = vault();
        let a = addr(0x05);
        v.stake(a, VALIDATOR_MIN_STAKE, 1).unwrap();

        let unstake_amount = VALIDATOR_MIN_STAKE / 2;
        v.begin_unstake(&a, unstake_amount, 1).unwrap();

        // Before period ends — nothing released
        let released_early = v.complete_unbonding(&a, 50);
        assert_eq!(released_early, 0);
        assert_eq!(v.get_stake(&a).unwrap().unbonding.len(), 1);

        // At completion height — full amount released
        let released = v.complete_unbonding(&a, 101); // 1 + 100
        assert_eq!(released, unstake_amount);
        assert!(v.get_stake(&a).unwrap().unbonding.is_empty());
    }

    #[test]
    fn test_unstake_more_than_staked_fails() {
        let mut v = vault();
        let a = addr(0x06);
        v.stake(a, VALIDATOR_MIN_STAKE, 1).unwrap();

        let too_much = VALIDATOR_MIN_STAKE + 1;
        let result = v.begin_unstake(&a, too_much, 10);
        assert_eq!(
            result,
            Err(StakingError::InsufficientBalance {
                requested: too_much,
                available: VALIDATOR_MIN_STAKE,
            })
        );
        // Stake unchanged
        assert_eq!(v.get_stake(&a).unwrap().amount, VALIDATOR_MIN_STAKE);
        assert_eq!(v.get_total_staked(), VALIDATOR_MIN_STAKE);
    }

    #[test]
    fn test_slash() {
        let mut v = vault();
        let a = addr(0x07);
        v.stake(a, VALIDATOR_MIN_STAKE, 1).unwrap();

        // Slash 10% (1000 bps)
        let slashed = v.slash(&a, 1000).unwrap();
        let expected = VALIDATOR_MIN_STAKE / 10;
        assert_eq!(slashed, expected);
        assert_eq!(v.get_stake(&a).unwrap().amount, VALIDATOR_MIN_STAKE - expected);
        assert_eq!(v.get_total_staked(), VALIDATOR_MIN_STAKE - expected);
    }

    #[test]
    fn test_slash_max_cap() {
        let mut v = vault();
        let a = addr(0x08);
        v.stake(a, VALIDATOR_MIN_STAKE, 1).unwrap();

        // Request 80% slash (8000 bps) — should be capped at 50% (5000 bps)
        let slashed = v.slash(&a, 8000).unwrap();
        let expected = VALIDATOR_MIN_STAKE / 2;
        assert_eq!(slashed, expected);
        assert_eq!(
            v.get_stake(&a).unwrap().amount,
            VALIDATOR_MIN_STAKE - expected
        );
    }

    #[test]
    fn test_total_staked_tracking() {
        let mut v = vault();
        let a1 = addr(0x11);
        let a2 = addr(0x22);

        v.stake(a1, VALIDATOR_MIN_STAKE, 1).unwrap();
        v.stake(a2, VALIDATOR_MIN_STAKE * 2, 2).unwrap();
        assert_eq!(v.get_total_staked(), VALIDATOR_MIN_STAKE * 3);

        let unstake = VALIDATOR_MIN_STAKE / 2;
        v.begin_unstake(&a1, unstake, 5).unwrap();
        assert_eq!(v.get_total_staked(), VALIDATOR_MIN_STAKE * 3 - unstake);

        v.complete_unbonding(&a1, 200);
        // complete_unbonding releases funds to caller; total_staked was already
        // decremented at begin_unstake time, so it should remain the same.
        assert_eq!(v.get_total_staked(), VALIDATOR_MIN_STAKE * 3 - unstake);
    }

    #[test]
    fn test_no_stake_found() {
        let mut v = vault();
        let a = addr(0xFF);

        assert_eq!(v.add_stake(&a, 1_000), Err(StakingError::NoStakeFound));
        assert_eq!(
            v.begin_unstake(&a, 1_000, 1),
            Err(StakingError::NoStakeFound)
        );
        assert_eq!(v.slash(&a, 100), Err(StakingError::NoStakeFound));
        assert_eq!(v.complete_unbonding(&a, 1000), 0);
    }
}
