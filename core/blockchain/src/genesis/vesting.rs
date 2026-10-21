use super::types::AllocationPurpose;
use crate::types::{Address, Amount, BlockHeight};
use serde::{Deserialize, Serialize};

/// Linear vesting schedule with an optional cliff.
///
/// No tokens vest before `cliff_height`.  Between `cliff_height` and
/// `end_height` vesting is linear (proportional to elapsed blocks).
/// At and after `end_height` the full `total_amount` is vested.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VestingSchedule {
    pub beneficiary: Address,
    pub total_amount: Amount,
    /// Tokens already released to the beneficiary.
    pub released: Amount,
    pub start_height: BlockHeight,
    /// No tokens vest before this height.
    pub cliff_height: BlockHeight,
    /// Fully vested at (and after) this height.
    pub end_height: BlockHeight,
    pub purpose: AllocationPurpose,
}

impl VestingSchedule {
    /// How much has vested at `current_height` (regardless of what has
    /// already been released).
    pub fn vested_amount(&self, current_height: BlockHeight) -> Amount {
        if current_height < self.cliff_height {
            return 0;
        }

        if current_height >= self.end_height {
            return self.total_amount;
        }

        // Linear vesting between start_height and end_height.
        // Use the later of start_height / cliff_height as the vesting
        // start so that the cliff jump is immediate.
        let vesting_start = self.start_height.max(self.cliff_height);

        if self.end_height <= vesting_start {
            // Degenerate schedule — treat as fully vested at cliff.
            return self.total_amount;
        }

        let elapsed = current_height.saturating_sub(vesting_start) as u128;
        let total_duration = (self.end_height - vesting_start) as u128;

        // Use u128 arithmetic to avoid overflow on large amounts.
        self.total_amount
            .saturating_mul(elapsed)
            / total_duration
    }

    /// Tokens that have vested but have not yet been released.
    pub fn releasable(&self, current_height: BlockHeight) -> Amount {
        self.vested_amount(current_height)
            .saturating_sub(self.released)
    }

    /// Release all currently releasable tokens.  Updates `self.released`
    /// and returns the released amount.
    pub fn release(&mut self, current_height: BlockHeight) -> Amount {
        let amount = self.releasable(current_height);
        self.released = self.released.saturating_add(amount);
        amount
    }

    /// Returns `true` once all tokens have vested.
    pub fn is_fully_vested(&self, current_height: BlockHeight) -> bool {
        current_height >= self.end_height
    }
}
