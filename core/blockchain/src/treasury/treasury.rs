use crate::types::{Address, Amount, BlockHeight};

use super::types::{Distribution, TreasuryError};

/// Protocol treasury that collects fees and distributes rewards to stakers.
///
/// Fees are charged at `fee_rate_bps` basis points on gross transaction
/// amounts (default `PROTOCOL_FEE_PERCENT` = 250 = 2.5%).  Accumulated
/// funds can be distributed proportionally or in fixed amounts by the
/// treasury admin.
pub struct ProtocolTreasury {
    /// Current spendable treasury balance
    balance: Amount,
    /// Lifetime fees collected
    total_collected: Amount,
    /// Lifetime rewards distributed
    total_distributed: Amount,
    /// Fee rate in basis points (250 = 2.5%)
    fee_rate_bps: u32,
    /// History of all distribution events
    distributions: Vec<Distribution>,
    /// Address of the treasury admin (only address allowed to distribute)
    admin: Address,
    /// Share of split distributions going to stakers, in basis points (default 4000 = 40%)
    pub staker_share_bps: u32,
    /// Share of split distributions retained by treasury, in basis points (default 6000 = 60%)
    pub treasury_share_bps: u32,
}

impl ProtocolTreasury {
    // ----------------------------------------------------------------
    // Construction
    // ----------------------------------------------------------------

    /// Create a new treasury with the given fee rate and admin address.
    ///
    /// Returns an error if `fee_rate_bps` exceeds 10 000 (100 %).
    pub fn new(fee_rate_bps: u32, admin: Address) -> Self {
        ProtocolTreasury {
            balance: 0,
            total_collected: 0,
            total_distributed: 0,
            fee_rate_bps,
            distributions: Vec::new(),
            admin,
            staker_share_bps: 4000,
            treasury_share_bps: 6000,
        }
    }

    // ----------------------------------------------------------------
    // Fee collection
    // ----------------------------------------------------------------

    /// Calculate the fee on `gross_amount` without mutating state.
    ///
    /// Uses integer arithmetic: `fee = gross * rate / 10_000`.
    pub fn calculate_fee(&self, gross_amount: Amount) -> Amount {
        gross_amount * self.fee_rate_bps as Amount / 10_000
    }

    /// Collect a protocol fee from `gross_amount`.
    ///
    /// Calculates `fee = gross * fee_rate_bps / 10_000`, adds it to the
    /// treasury balance and lifetime counters, and returns the fee amount.
    ///
    /// Returns `TreasuryError::ZeroAmount` if `gross_amount` is zero **or**
    /// if the resulting fee rounds down to zero.
    pub fn collect_fee(&mut self, gross_amount: Amount) -> Result<Amount, TreasuryError> {
        if gross_amount == 0 {
            return Err(TreasuryError::ZeroAmount);
        }
        let fee = self.calculate_fee(gross_amount);
        if fee == 0 {
            return Err(TreasuryError::ZeroAmount);
        }
        self.balance += fee;
        self.total_collected += fee;
        Ok(fee)
    }

    /// Directly deposit `amount` into the treasury (e.g. slashing proceeds).
    ///
    /// Returns `TreasuryError::ZeroAmount` if `amount` is zero.
    pub fn deposit(&mut self, amount: Amount) -> Result<(), TreasuryError> {
        if amount == 0 {
            return Err(TreasuryError::ZeroAmount);
        }
        self.balance += amount;
        self.total_collected += amount;
        Ok(())
    }

    // ----------------------------------------------------------------
    // Distribution
    // ----------------------------------------------------------------

    /// Distribute fixed amounts to a list of recipients.
    ///
    /// Only the treasury `admin` may call this.  The sum of all recipient
    /// amounts must not exceed the current balance.
    ///
    /// Returns the recorded `Distribution` on success.
    pub fn distribute(
        &mut self,
        recipients: Vec<(Address, Amount)>,
        height: BlockHeight,
        admin: &Address,
    ) -> Result<Distribution, TreasuryError> {
        if *admin != self.admin {
            return Err(TreasuryError::UnauthorizedAdmin(*admin));
        }
        if recipients.is_empty() {
            return Err(TreasuryError::NoRecipients);
        }

        let total_amount: Amount = recipients.iter().map(|(_, a)| a).sum();
        if total_amount == 0 {
            return Err(TreasuryError::ZeroAmount);
        }
        if total_amount > self.balance {
            return Err(TreasuryError::InsufficientBalance);
        }

        self.balance -= total_amount;
        self.total_distributed += total_amount;

        let distribution = Distribution {
            recipients,
            total_amount,
            height,
        };
        self.distributions.push(distribution.clone());
        Ok(distribution)
    }

    /// Distribute `total_to_distribute` proportionally across `stakers` by
    /// stake weight.
    ///
    /// Each staker receives `floor(total_to_distribute * stake / total_stake)`.
    /// Any rounding remainder is left in the treasury.
    ///
    /// Only the treasury `admin` may call this.
    pub fn distribute_proportional(
        &mut self,
        stakers: &[(Address, Amount)],
        total_to_distribute: Amount,
        height: BlockHeight,
        admin: &Address,
    ) -> Result<Distribution, TreasuryError> {
        if *admin != self.admin {
            return Err(TreasuryError::UnauthorizedAdmin(*admin));
        }
        if stakers.is_empty() {
            return Err(TreasuryError::NoRecipients);
        }
        if total_to_distribute == 0 {
            return Err(TreasuryError::ZeroAmount);
        }
        if total_to_distribute > self.balance {
            return Err(TreasuryError::InsufficientBalance);
        }

        let total_stake: Amount = stakers.iter().map(|(_, s)| s).sum();
        if total_stake == 0 {
            return Err(TreasuryError::ZeroAmount);
        }

        let recipients: Vec<(Address, Amount)> = stakers
            .iter()
            .filter_map(|(addr, stake)| {
                let share = total_to_distribute * stake / total_stake;
                if share > 0 {
                    Some((*addr, share))
                } else {
                    None
                }
            })
            .collect();

        if recipients.is_empty() {
            return Err(TreasuryError::ZeroAmount);
        }

        let actual_total: Amount = recipients.iter().map(|(_, a)| a).sum();

        self.balance -= actual_total;
        self.total_distributed += actual_total;

        // Normalise the stored total to reflect any rounding remainder
        let distribution = Distribution {
            recipients,
            total_amount: actual_total,
            height,
        };
        self.distributions.push(distribution.clone());
        Ok(distribution)
    }

    /// Distribute the current treasury balance using the 60/40 split:
    /// 40% is distributed proportionally to `stakers`; 60% is retained.
    ///
    /// Returns `(staker_distribution, treasury_retention_record)`.
    /// Only the treasury `admin` may call this.
    pub fn distribute_with_split(
        &mut self,
        stakers: &[(Address, Amount)],
        height: BlockHeight,
        admin: &Address,
    ) -> Result<(Distribution, Distribution), TreasuryError> {
        if *admin != self.admin {
            return Err(TreasuryError::UnauthorizedAdmin(*admin));
        }
        if stakers.is_empty() {
            return Err(TreasuryError::NoRecipients);
        }
        if self.balance == 0 {
            return Err(TreasuryError::InsufficientBalance);
        }

        let total_balance = self.balance;
        let staker_total = total_balance * self.staker_share_bps as Amount / 10_000;
        let treasury_retained = total_balance - staker_total;

        if staker_total == 0 {
            return Err(TreasuryError::ZeroAmount);
        }

        let total_stake: Amount = stakers.iter().map(|(_, s)| s).sum();
        if total_stake == 0 {
            return Err(TreasuryError::ZeroAmount);
        }

        let staker_recipients: Vec<(Address, Amount)> = stakers
            .iter()
            .filter_map(|(addr, stake)| {
                let share = staker_total * stake / total_stake;
                if share > 0 {
                    Some((*addr, share))
                } else {
                    None
                }
            })
            .collect();

        if staker_recipients.is_empty() {
            return Err(TreasuryError::ZeroAmount);
        }

        let actual_staker_total: Amount = staker_recipients.iter().map(|(_, a)| a).sum();

        // Deduct the staker portion from balance (treasury retention stays in balance)
        self.balance -= actual_staker_total;
        self.total_distributed += actual_staker_total;

        let staker_distribution = Distribution {
            recipients: staker_recipients,
            total_amount: actual_staker_total,
            height,
        };
        self.distributions.push(staker_distribution.clone());

        // Record treasury retention as a distribution to the admin address
        let treasury_retention = Distribution {
            recipients: vec![(self.admin, treasury_retained)],
            total_amount: treasury_retained,
            height,
        };

        Ok((staker_distribution, treasury_retention))
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    /// Current treasury balance available for distribution.
    pub fn get_balance(&self) -> Amount {
        self.balance
    }

    /// Lifetime total fees collected (including direct deposits).
    pub fn get_total_collected(&self) -> Amount {
        self.total_collected
    }

    /// Lifetime total rewards distributed.
    pub fn get_total_distributed(&self) -> Amount {
        self.total_distributed
    }

    /// Current fee rate in basis points.
    pub fn get_fee_rate(&self) -> u32 {
        self.fee_rate_bps
    }

    /// History of all distribution events.
    pub fn get_distributions(&self) -> &[Distribution] {
        &self.distributions
    }

    // ----------------------------------------------------------------
    // Admin: update fee rate
    // ----------------------------------------------------------------

    /// Update the fee rate.  Admin only; rate must be <= 10 000 bps.
    pub fn set_fee_rate(&mut self, new_rate: u32, admin: &Address) -> Result<(), TreasuryError> {
        if *admin != self.admin {
            return Err(TreasuryError::UnauthorizedAdmin(*admin));
        }
        if new_rate > 10_000 {
            return Err(TreasuryError::InvalidFeeRate(new_rate));
        }
        self.fee_rate_bps = new_rate;
        Ok(())
    }
}
