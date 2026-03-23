use crate::types::{Address, Amount, BlockHeight};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// VeISAError
// ============================================================================

/// Errors related to vote-escrowed ISA operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum VeISAError {
    #[error("Insufficient amount: minimum required is not met")]
    InsufficientAmount,

    #[error("Lock has not yet expired; cannot withdraw before lock_end")]
    LockNotExpired,

    #[error("No lock found for address {0:?}")]
    NoLockFound(Address),

    #[error("Lock already exists for address {0:?}; use extend_lock or increase_amount")]
    LockAlreadyExists(Address),

    #[error("Invalid lock duration: must be between min and max lock duration")]
    InvalidDuration,

    #[error("Amount must be greater than zero")]
    ZeroAmount,
}

// ============================================================================
// VeISALock
// ============================================================================

/// A single vote-escrowed ISA lock record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VeISALock {
    /// The owner of this lock.
    pub owner: Address,
    /// Amount of ISA locked.
    pub amount: Amount,
    /// Block height when the lock was created.
    pub lock_start: BlockHeight,
    /// Block height when the lock expires.
    pub lock_end: BlockHeight,
    /// Voting power at the time of the last calculation.
    pub voting_power: Amount,
    /// Block height at which voting_power was last calculated.
    pub last_calculated: BlockHeight,
}

// ============================================================================
// VeISASystem
// ============================================================================

/// Vote-escrowed ISA system — users lock ISA for a duration to earn governance
/// voting power.  Longer locks yield more power; power decays linearly to zero
/// as the lock approaches its expiry.
pub struct VeISASystem {
    /// Per-address lock records.
    locks: HashMap<Address, VeISALock>,
    /// Sum of all ISA currently locked (excluding withdrawn).
    total_locked: Amount,
    /// Cached sum of voting power (may be stale; use `get_total_voting_power`
    /// with a current height for an accurate value).
    total_voting_power: Amount,
    /// Minimum lock duration in blocks (default ≈ 1 000 blocks ≈ 50 min at 3 s/block).
    pub min_lock_duration: u64,
    /// Maximum lock duration in blocks (default ≈ 2 102 400 blocks ≈ 2 years).
    pub max_lock_duration: u64,
    /// Minimum ISA that can be locked (default 1 ISA in base units).
    pub min_amount: Amount,
}

impl VeISASystem {
    // ----------------------------------------------------------------
    // Constructor
    // ----------------------------------------------------------------

    /// Create a new VeISA system.
    ///
    /// * `min_lock` — minimum lock duration in blocks.
    /// * `max_lock` — maximum lock duration in blocks.
    /// * `min_amount` — minimum ISA amount that can be locked.
    pub fn new(min_lock: u64, max_lock: u64, min_amount: Amount) -> Self {
        VeISASystem {
            locks: HashMap::new(),
            total_locked: 0,
            total_voting_power: 0,
            min_lock_duration: min_lock,
            max_lock_duration: max_lock,
            min_amount,
        }
    }

    /// Convenience constructor using protocol defaults:
    /// * `min_lock_duration` = 1 000 blocks
    /// * `max_lock_duration` = 2 102 400 blocks
    /// * `min_amount` = 1 ISA (1 × 10^18 base units)
    pub fn default_system() -> Self {
        const ISA: Amount = 1_000_000_000_000_000_000; // 1 ISA in wei
        Self::new(1_000, 2_102_400, ISA)
    }

    // ----------------------------------------------------------------
    // Voting power formula (static helper)
    // ----------------------------------------------------------------

    /// Calculate voting power for a given locked amount and remaining time.
    ///
    /// Formula: `power = amount * remaining_blocks / max_duration`
    ///
    /// Returns 0 if `remaining_blocks` is 0 (lock expired).
    pub fn calculate_voting_power(
        amount: Amount,
        remaining_blocks: u64,
        max_duration: u64,
    ) -> Amount {
        if remaining_blocks == 0 || max_duration == 0 {
            return 0;
        }
        // Use u128 arithmetic throughout (Amount is u128).
        amount
            .saturating_mul(remaining_blocks as u128)
            / (max_duration as u128)
    }

    // ----------------------------------------------------------------
    // Lock
    // ----------------------------------------------------------------

    /// Lock `amount` ISA for `duration` blocks, starting at `height`.
    ///
    /// Returns the initial voting power on success.
    ///
    /// # Errors
    /// * `ZeroAmount` — `amount` == 0
    /// * `InsufficientAmount` — `amount` < `self.min_amount`
    /// * `InvalidDuration` — `duration` outside `[min_lock_duration, max_lock_duration]`
    /// * `LockAlreadyExists` — address already has an active lock
    pub fn lock(
        &mut self,
        owner: Address,
        amount: Amount,
        duration: u64,
        height: BlockHeight,
    ) -> Result<Amount, VeISAError> {
        if amount == 0 {
            return Err(VeISAError::ZeroAmount);
        }
        if amount < self.min_amount {
            return Err(VeISAError::InsufficientAmount);
        }
        if duration < self.min_lock_duration || duration > self.max_lock_duration {
            return Err(VeISAError::InvalidDuration);
        }
        if self.locks.contains_key(&owner) {
            return Err(VeISAError::LockAlreadyExists(owner));
        }

        let lock_end = height + duration;
        let voting_power =
            Self::calculate_voting_power(amount, duration, self.max_lock_duration);

        self.locks.insert(
            owner,
            VeISALock {
                owner,
                amount,
                lock_start: height,
                lock_end,
                voting_power,
                last_calculated: height,
            },
        );

        self.total_locked += amount;
        self.total_voting_power += voting_power;

        Ok(voting_power)
    }

    // ----------------------------------------------------------------
    // Extend lock
    // ----------------------------------------------------------------

    /// Extend an existing lock so it expires at `new_end`.
    ///
    /// `new_end` must be strictly after the current `lock_end`.
    /// The extension must not exceed `max_lock_duration` from the current height.
    ///
    /// Returns the updated voting power.
    ///
    /// # Errors
    /// * `NoLockFound` — address has no existing lock
    /// * `InvalidDuration` — `new_end` <= current `lock_end`, or total remaining
    ///   duration would exceed `max_lock_duration`
    pub fn extend_lock(
        &mut self,
        owner: &Address,
        new_end: BlockHeight,
        height: BlockHeight,
    ) -> Result<Amount, VeISAError> {
        let lock = self
            .locks
            .get_mut(owner)
            .ok_or(VeISAError::NoLockFound(*owner))?;

        if new_end <= lock.lock_end {
            return Err(VeISAError::InvalidDuration);
        }

        // Ensure new remaining duration doesn't exceed max.
        let new_remaining = new_end.saturating_sub(height);
        if new_remaining > self.max_lock_duration {
            return Err(VeISAError::InvalidDuration);
        }

        // Remove old voting power from total.
        let old_remaining = lock.lock_end.saturating_sub(height);
        let old_power =
            Self::calculate_voting_power(lock.amount, old_remaining, self.max_lock_duration);
        self.total_voting_power = self.total_voting_power.saturating_sub(old_power);

        // Apply new lock_end and recalculate.
        lock.lock_end = new_end;
        let new_power =
            Self::calculate_voting_power(lock.amount, new_remaining, self.max_lock_duration);
        lock.voting_power = new_power;
        lock.last_calculated = height;

        self.total_voting_power += new_power;

        Ok(new_power)
    }

    // ----------------------------------------------------------------
    // Increase amount
    // ----------------------------------------------------------------

    /// Add `additional` ISA to an existing lock.
    ///
    /// Returns the updated voting power.
    ///
    /// # Errors
    /// * `NoLockFound` — address has no existing lock
    /// * `ZeroAmount` — `additional` == 0
    pub fn increase_amount(
        &mut self,
        owner: &Address,
        additional: Amount,
    ) -> Result<Amount, VeISAError> {
        if additional == 0 {
            return Err(VeISAError::ZeroAmount);
        }

        let lock = self
            .locks
            .get_mut(owner)
            .ok_or(VeISAError::NoLockFound(*owner))?;

        // We can't recalculate without a current height, so we use the cached
        // remaining time from the last calculation epoch as a conservative
        // approximation.  The caller should follow up with get_voting_power to
        // get an accurate reading at the true current height.
        let remaining = lock.lock_end.saturating_sub(lock.last_calculated);
        let old_power =
            Self::calculate_voting_power(lock.amount, remaining, self.max_lock_duration);
        self.total_voting_power = self.total_voting_power.saturating_sub(old_power);
        self.total_locked = self.total_locked.saturating_sub(lock.amount);

        lock.amount += additional;
        self.total_locked += lock.amount;

        let new_power =
            Self::calculate_voting_power(lock.amount, remaining, self.max_lock_duration);
        lock.voting_power = new_power;
        self.total_voting_power += new_power;

        Ok(new_power)
    }

    // ----------------------------------------------------------------
    // Withdraw
    // ----------------------------------------------------------------

    /// Withdraw locked ISA after the lock has expired.
    ///
    /// Returns the withdrawn amount.
    ///
    /// # Errors
    /// * `NoLockFound` — address has no lock
    /// * `LockNotExpired` — `current_height` < `lock_end`
    pub fn withdraw(
        &mut self,
        owner: &Address,
        current_height: BlockHeight,
    ) -> Result<Amount, VeISAError> {
        let lock = self
            .locks
            .get(owner)
            .ok_or(VeISAError::NoLockFound(*owner))?;

        if current_height < lock.lock_end {
            return Err(VeISAError::LockNotExpired);
        }

        let withdrawn = lock.amount;

        // Remove stale voting power entry (power is 0 at expiry).
        let stale_power = lock.voting_power;
        self.total_voting_power = self.total_voting_power.saturating_sub(stale_power);
        self.total_locked = self.total_locked.saturating_sub(withdrawn);

        self.locks.remove(owner);

        Ok(withdrawn)
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    /// Current voting power for `owner`, decaying based on remaining lock time.
    ///
    /// Returns 0 if the lock has expired or does not exist.
    pub fn get_voting_power(&self, owner: &Address, current_height: BlockHeight) -> Amount {
        match self.locks.get(owner) {
            None => 0,
            Some(lock) => {
                if current_height >= lock.lock_end {
                    return 0;
                }
                let remaining = lock.lock_end - current_height;
                Self::calculate_voting_power(lock.amount, remaining, self.max_lock_duration)
            }
        }
    }

    /// Look up the raw lock record for `owner`.
    pub fn get_lock(&self, owner: &Address) -> Option<&VeISALock> {
        self.locks.get(owner)
    }

    /// Total ISA currently locked across all accounts.
    pub fn get_total_locked(&self) -> Amount {
        self.total_locked
    }

    /// Accurate total voting power summed across all active locks at
    /// `current_height`.
    pub fn get_total_voting_power(&self, current_height: BlockHeight) -> Amount {
        self.locks.values().fold(0_u128, |acc, lock| {
            acc + self.get_voting_power(&lock.owner, current_height)
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 1 ISA in base units (wei-equivalent).
    const ISA: Amount = 1_000_000_000_000_000_000;

    fn isa(n: u128) -> Amount {
        n * ISA
    }

    fn addr(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    /// VeISA system with protocol defaults.
    fn system() -> VeISASystem {
        VeISASystem::default_system()
    }

    // ----------------------------------------------------------------
    // test_lock
    // ----------------------------------------------------------------

    #[test]
    fn test_lock() {
        let mut sys = system();
        let owner = addr(0x01);
        let duration = 100_000_u64;
        let height = 1_000_u64;

        let power = sys.lock(owner, isa(10), duration, height).unwrap();
        assert!(power > 0, "voting power should be positive");

        let lock = sys.get_lock(&owner).unwrap();
        assert_eq!(lock.owner, owner);
        assert_eq!(lock.amount, isa(10));
        assert_eq!(lock.lock_start, height);
        assert_eq!(lock.lock_end, height + duration);

        assert_eq!(sys.get_total_locked(), isa(10));
    }

    // ----------------------------------------------------------------
    // test_lock_voting_power
    // ----------------------------------------------------------------

    #[test]
    fn test_lock_voting_power() {
        let mut sys = system();
        let max = sys.max_lock_duration;

        // Locking for the maximum duration should yield power == amount.
        let power_max = sys
            .lock(addr(0x01), isa(100), max, 0)
            .unwrap();
        assert_eq!(power_max, isa(100));

        // Locking for half the maximum duration should yield power == amount / 2.
        let power_half = sys
            .lock(addr(0x02), isa(100), max / 2, 0)
            .unwrap();
        assert_eq!(power_half, isa(50));
    }

    // ----------------------------------------------------------------
    // test_power_decay
    // ----------------------------------------------------------------

    #[test]
    fn test_power_decay() {
        let mut sys = system();
        let max = sys.max_lock_duration;
        let owner = addr(0x01);

        sys.lock(owner, isa(100), max, 0).unwrap();

        // At lock creation, power == amount.
        let power_at_0 = sys.get_voting_power(&owner, 0);
        assert_eq!(power_at_0, isa(100));

        // After half the duration has elapsed, power ≈ amount / 2.
        let power_at_half = sys.get_voting_power(&owner, max / 2);
        assert_eq!(power_at_half, isa(50));

        // After the full duration, power == 0 (lock expired).
        let power_expired = sys.get_voting_power(&owner, max);
        assert_eq!(power_expired, 0);
    }

    // ----------------------------------------------------------------
    // test_extend_lock
    // ----------------------------------------------------------------

    #[test]
    fn test_extend_lock() {
        let mut sys = system();
        let max = sys.max_lock_duration;
        let owner = addr(0x01);

        // Lock for 50 % of max.
        sys.lock(owner, isa(100), max / 2, 0).unwrap();
        let original_end = sys.get_lock(&owner).unwrap().lock_end;

        // Extend so the new end is at max from block 0.
        let new_end = max;
        let power_after = sys.extend_lock(&owner, new_end, 0).unwrap();

        let lock = sys.get_lock(&owner).unwrap();
        assert_eq!(lock.lock_end, new_end);
        assert!(lock.lock_end > original_end);

        // Power should now equal full amount (locked for max duration).
        assert_eq!(power_after, isa(100));
    }

    // ----------------------------------------------------------------
    // test_increase_amount
    // ----------------------------------------------------------------

    #[test]
    fn test_increase_amount() {
        let mut sys = system();
        let max = sys.max_lock_duration;
        let owner = addr(0x01);

        sys.lock(owner, isa(50), max, 0).unwrap();
        let power_before = sys.get_lock(&owner).unwrap().voting_power;

        sys.increase_amount(&owner, isa(50)).unwrap();

        let lock = sys.get_lock(&owner).unwrap();
        assert_eq!(lock.amount, isa(100));
        assert!(lock.voting_power > power_before);
        assert_eq!(sys.get_total_locked(), isa(100));
    }

    // ----------------------------------------------------------------
    // test_withdraw_after_expiry
    // ----------------------------------------------------------------

    #[test]
    fn test_withdraw_after_expiry() {
        let mut sys = system();
        let owner = addr(0x01);
        let duration = sys.min_lock_duration;

        sys.lock(owner, isa(100), duration, 0).unwrap();

        // Withdraw exactly at expiry.
        let withdrawn = sys.withdraw(&owner, duration).unwrap();
        assert_eq!(withdrawn, isa(100));

        // Lock should be gone.
        assert!(sys.get_lock(&owner).is_none());
        assert_eq!(sys.get_total_locked(), 0);
    }

    // ----------------------------------------------------------------
    // test_withdraw_before_expiry_fails
    // ----------------------------------------------------------------

    #[test]
    fn test_withdraw_before_expiry_fails() {
        let mut sys = system();
        let owner = addr(0x01);
        let duration = sys.min_lock_duration;

        sys.lock(owner, isa(100), duration, 0).unwrap();

        // One block before expiry.
        let err = sys.withdraw(&owner, duration - 1).unwrap_err();
        assert_eq!(err, VeISAError::LockNotExpired);

        // Lock should still be intact.
        assert!(sys.get_lock(&owner).is_some());
        assert_eq!(sys.get_total_locked(), isa(100));
    }

    // ----------------------------------------------------------------
    // test_duplicate_lock_fails
    // ----------------------------------------------------------------

    #[test]
    fn test_duplicate_lock_fails() {
        let mut sys = system();
        let owner = addr(0x01);
        let duration = sys.min_lock_duration;

        sys.lock(owner, isa(10), duration, 0).unwrap();

        let err = sys
            .lock(owner, isa(10), duration, 0)
            .unwrap_err();
        assert_eq!(err, VeISAError::LockAlreadyExists(owner));
    }

    // ----------------------------------------------------------------
    // test_invalid_duration
    // ----------------------------------------------------------------

    #[test]
    fn test_invalid_duration() {
        let mut sys = system();
        let owner_a = addr(0x01);
        let owner_b = addr(0x02);

        // Too short.
        let err = sys
            .lock(owner_a, isa(10), sys.min_lock_duration - 1, 0)
            .unwrap_err();
        assert_eq!(err, VeISAError::InvalidDuration);

        // Too long.
        let err = sys
            .lock(owner_b, isa(10), sys.max_lock_duration + 1, 0)
            .unwrap_err();
        assert_eq!(err, VeISAError::InvalidDuration);
    }

    // ----------------------------------------------------------------
    // test_zero_amount_fails
    // ----------------------------------------------------------------

    #[test]
    fn test_zero_amount_fails() {
        let mut sys = system();
        let owner = addr(0x01);
        let duration = sys.min_lock_duration;

        let err = sys.lock(owner, 0, duration, 0).unwrap_err();
        assert_eq!(err, VeISAError::ZeroAmount);
    }

    // ----------------------------------------------------------------
    // test_total_voting_power
    // ----------------------------------------------------------------

    #[test]
    fn test_total_voting_power() {
        let mut sys = system();
        let max = sys.max_lock_duration;

        // Two users each lock 100 ISA for max duration — combined power == 200 ISA.
        sys.lock(addr(0x01), isa(100), max, 0).unwrap();
        sys.lock(addr(0x02), isa(100), max, 0).unwrap();

        let total = sys.get_total_voting_power(0);
        assert_eq!(total, isa(200));

        // At half duration, total should be ≈ 100 ISA.
        let total_half = sys.get_total_voting_power(max / 2);
        assert_eq!(total_half, isa(100));
    }

    // ----------------------------------------------------------------
    // test_max_power_at_max_duration
    // ----------------------------------------------------------------

    #[test]
    fn test_max_power_at_max_duration() {
        let mut sys = system();
        let max = sys.max_lock_duration;
        let owner = addr(0x01);
        let amount = isa(500);

        let power = sys.lock(owner, amount, max, 0).unwrap();

        // Locking for the maximum duration must return power == amount.
        assert_eq!(power, amount);

        // get_voting_power at block 0 should also return amount.
        assert_eq!(sys.get_voting_power(&owner, 0), amount);
    }

    // ----------------------------------------------------------------
    // Additional edge-case: no_lock_found queries
    // ----------------------------------------------------------------

    #[test]
    fn test_no_lock_found() {
        let mut sys = system();
        let ghost = addr(0xFF);

        assert_eq!(sys.get_voting_power(&ghost, 100), 0);
        assert!(sys.get_lock(&ghost).is_none());

        assert_eq!(
            sys.extend_lock(&ghost, 9999, 100).unwrap_err(),
            VeISAError::NoLockFound(ghost)
        );
        assert_eq!(
            sys.increase_amount(&ghost, isa(1)).unwrap_err(),
            VeISAError::NoLockFound(ghost)
        );
        assert_eq!(
            sys.withdraw(&ghost, 999_999).unwrap_err(),
            VeISAError::NoLockFound(ghost)
        );
    }
}
