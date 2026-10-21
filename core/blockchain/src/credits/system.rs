use crate::types::{Address, Amount, BlockHeight};
use std::collections::HashMap;

use super::types::{CreditAccount, CreditError, DEFAULT_CREDIT_PRICE_USD};

const MICRO_USD_SCALE: Amount = 1_000_000;

// ====================================================================
// CreditSystem
// ====================================================================

/// On-chain ISA Credits system providing a stable payment unit pegged to USD.
///
/// Users purchase credits by spending ISA tokens.  The exchange rate is
/// determined by the ISA/USD price (provided externally, typically from the
/// [`PriceOracle`](crate::oracle::PriceOracle)).
///
/// ## Unit conventions
/// - **credit_price_usd**: price per *one credit* in micro-USD (default 100 = $0.00001)
/// - **isa_price_usd**: ISA/USD spot price in micro-USD (e.g. 500_000 = $0.50)
/// - **credit_balance**: stored in the same unit as `min_purchase` counts — i.e. whole
///   credits (not micro-credits); the docstring "micro-credits" in the struct field
///   refers to the spec naming; internally we treat balance as an integer credit count.
///
/// ## Conversion formula
/// ```text
/// credits = (isa_amount * isa_price_usd) / credit_price_usd
/// ```
/// Example: 1 ISA × $0.50/ISA / $0.00001/credit = 50_000 credits
pub struct CreditSystem {
    /// All credit accounts, keyed by address
    pub accounts: HashMap<Address, CreditAccount>,
    /// Price per credit in micro-USD (default 10_000 = $0.01)
    pub credit_price_usd: Amount,
    /// Lifetime credits issued (purchased + granted)
    pub total_credits_issued: Amount,
    /// Lifetime credits burned (spent)
    pub total_credits_burned: Amount,
    /// Minimum credits per purchase transaction
    pub min_purchase: Amount,
    /// Protocol admin address (can set price, grant credits)
    pub admin: Address,
}

impl CreditSystem {
    // ----------------------------------------------------------------
    // Construction
    // ----------------------------------------------------------------

    /// Create a new `CreditSystem`.
    ///
    /// - `credit_price_usd`: price per credit in micro-USD (use [`DEFAULT_CREDIT_PRICE_USD`] for $0.01)
    /// - `min_purchase`: minimum credits per purchase (use [`DEFAULT_MIN_PURCHASE`] for $1.00 minimum)
    /// - `admin`: the address allowed to call admin-only methods
    pub fn new(credit_price_usd: Amount, min_purchase: Amount, admin: Address) -> Self {
        CreditSystem {
            accounts: HashMap::new(),
            credit_price_usd,
            total_credits_issued: 0,
            total_credits_burned: 0,
            min_purchase,
            admin,
        }
    }

    // ----------------------------------------------------------------
    // Core operations
    // ----------------------------------------------------------------

    /// Purchase credits by spending ISA tokens.
    ///
    /// Converts `isa_amount` (in micro-ISA) to credits using the spot
    /// `isa_price_usd` (in micro-USD per ISA) and the system's
    /// `credit_price_usd`.
    ///
    /// Returns the number of credits credited to the account.
    ///
    /// Fails with:
    /// - [`CreditError::InvalidAmount`] if `isa_amount` is zero
    /// - [`CreditError::BelowMinimumPurchase`] if the resulting credits are below `min_purchase`
    pub fn purchase_credits(
        &mut self,
        address: Address,
        isa_amount: Amount,
        isa_price_usd: Amount,
        height: BlockHeight,
    ) -> Result<Amount, CreditError> {
        if isa_amount == 0 {
            return Err(CreditError::InvalidAmount);
        }

        let credits = Self::get_credits_for_isa(isa_amount, isa_price_usd, self.credit_price_usd);

        if credits < self.min_purchase {
            return Err(CreditError::BelowMinimumPurchase);
        }

        let account = self
            .accounts
            .entry(address)
            .or_insert_with(|| CreditAccount::new(address));

        account.credit_balance += credits;
        account.total_credits_purchased += credits;
        account.last_top_up_height = height;

        self.total_credits_issued += credits;

        Ok(credits)
    }

    /// Deduct `credits` from the account's balance for service usage.
    ///
    /// Fails with:
    /// - [`CreditError::AccountNotFound`] if the address has no account
    /// - [`CreditError::InsufficientCredits`] if the balance is too low
    pub fn spend_credits(&mut self, address: &Address, credits: Amount) -> Result<(), CreditError> {
        let account = self
            .accounts
            .get_mut(address)
            .ok_or(CreditError::AccountNotFound(*address))?;

        if account.credit_balance < credits {
            return Err(CreditError::InsufficientCredits);
        }

        account.credit_balance -= credits;
        account.total_credits_spent += credits;
        self.total_credits_burned += credits;

        Ok(())
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    /// Return the credit balance for `address`, or 0 if no account exists.
    pub fn get_balance(&self, address: &Address) -> Amount {
        self.accounts
            .get(address)
            .map(|a| a.credit_balance)
            .unwrap_or(0)
    }

    /// Return a reference to the [`CreditAccount`] for `address`, or `None`.
    pub fn get_account(&self, address: &Address) -> Option<&CreditAccount> {
        self.accounts.get(address)
    }

    /// Compute how much ISA (in micro-ISA) is required to obtain `credits`
    /// at the given `isa_price_usd` (micro-USD per ISA).
    ///
    /// Formula: `isa_needed = credits * credit_price_usd / isa_price_usd`
    ///
    /// Returns 0 when `isa_price_usd` is 0 (safe fallback).
    pub fn get_isa_cost_for_credits(
        credits: Amount,
        isa_price_usd: Amount,
    ) -> Amount {
        if isa_price_usd == 0 {
            return 0;
        }
        // credits * credit_price_usd / isa_price_usd
        // We use DEFAULT_CREDIT_PRICE_USD as the system-level price here;
        // callers that need a custom price should use the formula directly.
        // The method is static, so we use the default credit price.
        credits
            .checked_mul(DEFAULT_CREDIT_PRICE_USD)
            .map(|v| v / isa_price_usd)
            .unwrap_or(u128::MAX)
    }

    /// Compute how many credits are obtained for `isa_amount` (micro-ISA)
    /// at the given `isa_price_usd` (micro-USD per ISA) and this system's
    /// `credit_price_usd`.
    ///
    /// Formula: `credits = (isa_amount * isa_price_usd) / credit_price_usd`
    fn get_credits_for_isa(
        isa_amount: Amount,
        isa_price_usd: Amount,
        credit_price_usd: Amount,
    ) -> Amount {
        if credit_price_usd == 0 {
            return 0;
        }
        // isa_amount is in micro-ISA; isa_price_usd is micro-USD per *whole* ISA.
        // Normalize: usd_value_micro = isa_amount * isa_price_usd / MICRO_USD_SCALE
        // credits = usd_value_micro / credit_price_usd
        // Combined: credits = (isa_amount * isa_price_usd) / (credit_price_usd * MICRO_USD_SCALE)
        let denominator = credit_price_usd.saturating_mul(MICRO_USD_SCALE);
        isa_amount
            .checked_mul(isa_price_usd)
            .map(|v| v / denominator)
            .unwrap_or(u128::MAX)
    }

    /// Public static convenience wrapper for conversion math (uses system default credit price).
    ///
    /// Formula: `credits = (isa_amount * isa_price_usd) / (DEFAULT_CREDIT_PRICE_USD * MICRO_USD_SCALE)`
    pub fn credits_for_isa(isa_amount: Amount, isa_price_usd: Amount) -> Amount {
        Self::get_credits_for_isa(isa_amount, isa_price_usd, DEFAULT_CREDIT_PRICE_USD)
    }

    // ----------------------------------------------------------------
    // Admin operations
    // ----------------------------------------------------------------

    /// Update the credit price.  Only the admin may call this.
    pub fn set_credit_price(
        &mut self,
        new_price: Amount,
        admin: &Address,
    ) -> Result<(), CreditError> {
        self.check_admin(admin)?;
        if new_price == 0 {
            return Err(CreditError::InvalidAmount);
        }
        self.credit_price_usd = new_price;
        Ok(())
    }

    /// Grant credits to an address (e.g. for promotions).  Only the admin may call this.
    pub fn grant_credits(
        &mut self,
        address: Address,
        credits: Amount,
        admin: &Address,
        height: BlockHeight,
    ) -> Result<(), CreditError> {
        self.check_admin(admin)?;
        if credits == 0 {
            return Err(CreditError::InvalidAmount);
        }

        let account = self
            .accounts
            .entry(address)
            .or_insert_with(|| CreditAccount::new(address));

        account.credit_balance += credits;
        account.total_credits_purchased += credits;
        account.last_top_up_height = height;

        self.total_credits_issued += credits;

        Ok(())
    }

    // ----------------------------------------------------------------
    // Supply stats
    // ----------------------------------------------------------------

    /// Credits currently in circulation (issued minus burned).
    pub fn total_credits_in_circulation(&self) -> Amount {
        self.total_credits_issued.saturating_sub(self.total_credits_burned)
    }

    // ----------------------------------------------------------------
    // Private helpers
    // ----------------------------------------------------------------

    fn check_admin(&self, caller: &Address) -> Result<(), CreditError> {
        if *caller != self.admin {
            Err(CreditError::UnauthorizedAdmin(*caller))
        } else {
            Ok(())
        }
    }
}
