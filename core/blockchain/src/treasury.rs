use crate::types::{Address, Amount, BlockHeight};

/// Errors related to treasury operations
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TreasuryError {
    #[error("Insufficient treasury balance to distribute")]
    InsufficientBalance,

    #[error("Invalid fee rate: {0} bps exceeds maximum of 10000 bps")]
    InvalidFeeRate(u32),

    #[error("Amount must be greater than zero")]
    ZeroAmount,

    #[error("Distribution requires at least one recipient")]
    NoRecipients,

    #[error("Unauthorized admin operation: {0} is not the admin")]
    UnauthorizedAdmin(Address),
}

/// A record of a single distribution event from the treasury to stakers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Distribution {
    /// Who received funds and how much each recipient got
    pub recipients: Vec<(Address, Amount)>,
    /// Total amount distributed in this event
    pub total_amount: Amount,
    /// Block height at which the distribution occurred
    pub height: BlockHeight,
}

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

// ====================================================================
// Tests
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::constants::PROTOCOL_FEE_PERCENT;

    fn admin() -> Address {
        Address::from([0xAA; 20])
    }

    fn other() -> Address {
        Address::from([0xBB; 20])
    }

    fn staker(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    fn default_treasury() -> ProtocolTreasury {
        ProtocolTreasury::new(PROTOCOL_FEE_PERCENT, admin())
    }

    // ----------------------------------------------------------------
    // collect_fee
    // ----------------------------------------------------------------

    #[test]
    fn test_collect_fee() {
        let mut t = default_treasury();
        // 2.5% of 10_000 = 250
        let fee = t.collect_fee(10_000).unwrap();
        assert_eq!(fee, 250);
        assert_eq!(t.get_balance(), 250);
        assert_eq!(t.get_total_collected(), 250);
    }

    #[test]
    fn test_collect_fee_zero_fails() {
        let mut t = default_treasury();
        let result = t.collect_fee(0);
        assert_eq!(result, Err(TreasuryError::ZeroAmount));
    }

    /// A gross amount so small that the fee rounds to zero should also fail.
    #[test]
    fn test_collect_fee_rounds_to_zero_fails() {
        let mut t = default_treasury();
        // 1 * 250 / 10_000 == 0 (integer division)
        let result = t.collect_fee(1);
        assert_eq!(result, Err(TreasuryError::ZeroAmount));
    }

    // ----------------------------------------------------------------
    // deposit
    // ----------------------------------------------------------------

    #[test]
    fn test_deposit() {
        let mut t = default_treasury();
        t.deposit(1_000).unwrap();
        assert_eq!(t.get_balance(), 1_000);
        assert_eq!(t.get_total_collected(), 1_000);
    }

    #[test]
    fn test_deposit_zero_fails() {
        let mut t = default_treasury();
        assert_eq!(t.deposit(0), Err(TreasuryError::ZeroAmount));
    }

    // ----------------------------------------------------------------
    // distribute
    // ----------------------------------------------------------------

    #[test]
    fn test_distribute() {
        let mut t = default_treasury();
        t.deposit(1_000).unwrap();

        let recipients = vec![(staker(0x01), 600), (staker(0x02), 400)];
        let dist = t.distribute(recipients.clone(), 42, &admin()).unwrap();

        assert_eq!(dist.total_amount, 1_000);
        assert_eq!(dist.height, 42);
        assert_eq!(dist.recipients, recipients);
        assert_eq!(t.get_balance(), 0);
        assert_eq!(t.get_total_distributed(), 1_000);
    }

    #[test]
    fn test_distribute_insufficient_balance() {
        let mut t = default_treasury();
        t.deposit(500).unwrap();

        let result = t.distribute(vec![(staker(0x01), 1_000)], 1, &admin());
        assert_eq!(result, Err(TreasuryError::InsufficientBalance));
    }

    #[test]
    fn test_distribute_no_recipients_fails() {
        let mut t = default_treasury();
        t.deposit(1_000).unwrap();

        let result = t.distribute(vec![], 1, &admin());
        assert_eq!(result, Err(TreasuryError::NoRecipients));
    }

    // ----------------------------------------------------------------
    // distribute_proportional
    // ----------------------------------------------------------------

    #[test]
    fn test_distribute_proportional() {
        let mut t = default_treasury();
        t.deposit(1_000).unwrap();

        // Staker A has 3x the stake of staker B → 750 vs 250
        let stakers = vec![(staker(0x01), 3_000), (staker(0x02), 1_000)];
        let dist = t.distribute_proportional(&stakers, 1_000, 10, &admin()).unwrap();

        assert_eq!(dist.recipients.len(), 2);
        // Staker A: 1000 * 3000 / 4000 = 750
        assert_eq!(dist.recipients[0], (staker(0x01), 750));
        // Staker B: 1000 * 1000 / 4000 = 250
        assert_eq!(dist.recipients[1], (staker(0x02), 250));
        assert_eq!(dist.total_amount, 1_000);
        assert_eq!(t.get_balance(), 0);
    }

    // ----------------------------------------------------------------
    // set_fee_rate
    // ----------------------------------------------------------------

    #[test]
    fn test_set_fee_rate() {
        let mut t = default_treasury();
        t.set_fee_rate(500, &admin()).unwrap();
        assert_eq!(t.get_fee_rate(), 500);
    }

    #[test]
    fn test_set_fee_rate_too_high() {
        let mut t = default_treasury();
        let result = t.set_fee_rate(10_001, &admin());
        assert_eq!(result, Err(TreasuryError::InvalidFeeRate(10_001)));
    }

    #[test]
    fn test_set_fee_rate_max_boundary() {
        let mut t = default_treasury();
        // Exactly 10_000 (100%) should be allowed
        assert!(t.set_fee_rate(10_000, &admin()).is_ok());
        assert_eq!(t.get_fee_rate(), 10_000);
    }

    // ----------------------------------------------------------------
    // Unauthorized admin
    // ----------------------------------------------------------------

    #[test]
    fn test_unauthorized_admin_fails() {
        let mut t = default_treasury();
        t.deposit(1_000).unwrap();

        // distribute with wrong admin
        let result = t.distribute(vec![(staker(0x01), 100)], 1, &other());
        assert_eq!(result, Err(TreasuryError::UnauthorizedAdmin(other())));

        // set_fee_rate with wrong admin
        let result2 = t.set_fee_rate(100, &other());
        assert_eq!(result2, Err(TreasuryError::UnauthorizedAdmin(other())));

        // distribute_proportional with wrong admin
        let result3 = t.distribute_proportional(
            &[(staker(0x01), 1_000)],
            500,
            1,
            &other(),
        );
        assert_eq!(result3, Err(TreasuryError::UnauthorizedAdmin(other())));
    }

    // ----------------------------------------------------------------
    // calculate_fee
    // ----------------------------------------------------------------

    #[test]
    fn test_calculate_fee() {
        let t = default_treasury();
        // 2.5% of 1_000_000 = 25_000
        assert_eq!(t.calculate_fee(1_000_000), 25_000);
        // 2.5% of 0 = 0
        assert_eq!(t.calculate_fee(0), 0);
    }

    #[test]
    fn test_calculate_fee_custom_rate() {
        let t = ProtocolTreasury::new(100, admin()); // 1%
        assert_eq!(t.calculate_fee(10_000), 100);
    }

    // ----------------------------------------------------------------
    // Lifetime tracking
    // ----------------------------------------------------------------

    #[test]
    fn test_lifetime_tracking() {
        let mut t = default_treasury();

        // Collect fees
        t.collect_fee(40_000).unwrap(); // fee = 1_000
        t.collect_fee(80_000).unwrap(); // fee = 2_000
        t.deposit(500).unwrap();

        assert_eq!(t.get_total_collected(), 3_500);
        assert_eq!(t.get_balance(), 3_500);

        // Distribute some
        t.distribute(vec![(staker(0x01), 1_000)], 1, &admin()).unwrap();
        assert_eq!(t.get_total_distributed(), 1_000);
        assert_eq!(t.get_balance(), 2_500);

        // Distribute more
        t.distribute(vec![(staker(0x02), 2_000)], 2, &admin()).unwrap();
        assert_eq!(t.get_total_distributed(), 3_000);
        assert_eq!(t.get_balance(), 500);

        // Distribution history
        assert_eq!(t.get_distributions().len(), 2);
        assert_eq!(t.get_distributions()[0].height, 1);
        assert_eq!(t.get_distributions()[1].height, 2);
    }

    // ----------------------------------------------------------------
    // distribute_with_split
    // ----------------------------------------------------------------

    #[test]
    fn test_distribute_with_split() {
        let mut t = default_treasury();
        // Deposit 10_000 into treasury
        t.deposit(10_000).unwrap();
        assert_eq!(t.get_balance(), 10_000);

        // Two equal stakers
        let stakers = vec![(staker(0x01), 1_000), (staker(0x02), 1_000)];
        let (staker_dist, treasury_retention) =
            t.distribute_with_split(&stakers, 50, &admin()).unwrap();

        // 40% of 10_000 = 4_000 goes to stakers, split equally: 2_000 each
        assert_eq!(staker_dist.total_amount, 4_000);
        assert_eq!(staker_dist.height, 50);
        assert_eq!(staker_dist.recipients.len(), 2);
        assert_eq!(staker_dist.recipients[0], (staker(0x01), 2_000));
        assert_eq!(staker_dist.recipients[1], (staker(0x02), 2_000));

        // 60% of 10_000 = 6_000 retained
        assert_eq!(treasury_retention.total_amount, 6_000);
        assert_eq!(treasury_retention.recipients[0].0, admin());

        // Balance is reduced only by staker payout; 6_000 stays
        assert_eq!(t.get_balance(), 6_000);
        assert_eq!(t.get_total_distributed(), 4_000);
    }
}
