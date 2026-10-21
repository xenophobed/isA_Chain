use crate::types::{Address, Amount, BlockHeight};
use std::collections::HashMap;

use super::types::{VeISAError, VeISALock};

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
