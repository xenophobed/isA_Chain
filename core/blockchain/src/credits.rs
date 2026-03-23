use crate::types::{Address, Amount, BlockHeight};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ====================================================================
// Constants
// ====================================================================

/// Default credit price: $0.00001 USD = 100 micro-USD
pub const DEFAULT_CREDIT_PRICE_USD: Amount = 100;

/// Default minimum purchase: 100 credits = $0.001 worth
pub const DEFAULT_MIN_PURCHASE: Amount = 100;

/// Micro-USD scale factor: 1_000_000 micro-USD = $1.00
const MICRO_USD_SCALE: Amount = 1_000_000;

// ====================================================================
// Errors
// ====================================================================

/// Errors related to credit system operations
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CreditError {
    #[error("Insufficient credits: not enough credits to complete operation")]
    InsufficientCredits,

    #[error("Insufficient ISA: not enough ISA tokens to purchase credits")]
    InsufficientISA,

    #[error("Below minimum purchase: must purchase at least the minimum credit amount")]
    BelowMinimumPurchase,

    #[error("Invalid amount: amount must be greater than zero")]
    InvalidAmount,

    #[error("Account not found: {0}")]
    AccountNotFound(Address),

    #[error("Unauthorized admin: {0} is not the admin")]
    UnauthorizedAdmin(Address),
}

// ====================================================================
// CreditAccount
// ====================================================================

/// An account in the ISA Credits system.
///
/// Credits are the stable payment unit for platform services, pegged to USD.
/// 1 credit = $0.01 USD = 1_000_000 micro-credits in the base unit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreditAccount {
    /// Account address
    pub address: Address,
    /// Balance in micro-credits (1_000_000 = 1 credit = $0.01 USD)
    pub credit_balance: Amount,
    /// Cumulative credits purchased over account lifetime
    pub total_credits_purchased: Amount,
    /// Cumulative credits spent over account lifetime
    pub total_credits_spent: Amount,
    /// Block height of the most recent top-up
    pub last_top_up_height: BlockHeight,
}

impl CreditAccount {
    fn new(address: Address) -> Self {
        CreditAccount {
            address,
            credit_balance: 0,
            total_credits_purchased: 0,
            total_credits_spent: 0,
            last_top_up_height: 0,
        }
    }
}

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

// ====================================================================
// Tests
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------------
    // Fixtures
    // ----------------------------------------------------------------

    fn admin() -> Address {
        Address::from([0xAA; 20])
    }

    fn user() -> Address {
        Address::from([0xBB; 20])
    }

    fn user2() -> Address {
        Address::from([0xCC; 20])
    }

    fn random_addr() -> Address {
        Address::from([0xDD; 20])
    }

    /// ISA price: $0.50 = 500_000 micro-USD
    const ISA_PRICE: Amount = 500_000;

    fn setup() -> CreditSystem {
        CreditSystem::new(DEFAULT_CREDIT_PRICE_USD, DEFAULT_MIN_PURCHASE, admin())
    }

    // ----------------------------------------------------------------
    // test_purchase_credits
    // ----------------------------------------------------------------

    #[test]
    fn test_purchase_credits() {
        let mut cs = setup();

        // Purchase with 2 ISA at $0.50/ISA
        // credits = (2_000_000 * 500_000) / (100 * 1_000_000) = 10_000 credits
        // min_purchase = 100, so well above the minimum — should succeed
        let isa_amount: Amount = 2_000_000; // 2 ISA in micro-ISA
        let credits = cs
            .purchase_credits(user(), isa_amount, ISA_PRICE, 10)
            .unwrap();

        assert_eq!(credits, 10_000);
        assert_eq!(cs.get_balance(&user()), 10_000);
        assert_eq!(cs.total_credits_issued, 10_000);
    }

    // ----------------------------------------------------------------
    // test_purchase_credits_conversion_math
    // ----------------------------------------------------------------

    #[test]
    fn test_purchase_credits_conversion_math() {
        let mut cs = setup();

        // 2 ISA at $0.50/ISA → 10_000 credits
        let credits = cs
            .purchase_credits(user(), 2_000_000, ISA_PRICE, 1)
            .unwrap();
        assert_eq!(credits, 10_000);

        // 10 ISA at $1.00/ISA → 100_000 credits
        let credits2 = cs
            .purchase_credits(user2(), 10_000_000, 1_000_000, 2)
            .unwrap();
        assert_eq!(credits2, 100_000);
    }

    // ----------------------------------------------------------------
    // test_spend_credits
    // ----------------------------------------------------------------

    #[test]
    fn test_spend_credits() {
        let mut cs = setup();
        cs.purchase_credits(user(), 2_000_000, ISA_PRICE, 1)
            .unwrap(); // 10_000 credits

        cs.spend_credits(&user(), 30).unwrap();

        assert_eq!(cs.get_balance(&user()), 9_970);
        assert_eq!(cs.total_credits_burned, 30);

        let acct = cs.get_account(&user()).unwrap();
        assert_eq!(acct.total_credits_spent, 30);
    }

    // ----------------------------------------------------------------
    // test_spend_insufficient_fails
    // ----------------------------------------------------------------

    #[test]
    fn test_spend_insufficient_fails() {
        let mut cs = setup();
        cs.purchase_credits(user(), 2_000_000, ISA_PRICE, 1)
            .unwrap(); // 10_000 credits

        let result = cs.spend_credits(&user(), 20_000);
        assert_eq!(result, Err(CreditError::InsufficientCredits));

        // Balance must be unchanged
        assert_eq!(cs.get_balance(&user()), 10_000);
    }

    // ----------------------------------------------------------------
    // test_below_minimum_purchase
    // ----------------------------------------------------------------

    #[test]
    fn test_below_minimum_purchase() {
        let mut cs = setup();

        // With min_purchase = 100, ISA price = $0.50, credit_price = 100 micro-USD
        // credits = (isa_amount * 500_000) / (100 * 1_000_000) = isa_amount / 200
        // Need at least 100 credits → isa_amount >= 20_000 micro-ISA
        // Send only 10_000 micro-ISA → 50 credits < 100 minimum
        let result = cs.purchase_credits(user(), 10_000, ISA_PRICE, 1);
        assert_eq!(result, Err(CreditError::BelowMinimumPurchase));
    }

    // ----------------------------------------------------------------
    // test_get_balance_no_account
    // ----------------------------------------------------------------

    #[test]
    fn test_get_balance_no_account() {
        let cs = setup();
        assert_eq!(cs.get_balance(&random_addr()), 0);
        assert!(cs.get_account(&random_addr()).is_none());
    }

    // ----------------------------------------------------------------
    // test_get_isa_cost_for_credits
    // ----------------------------------------------------------------

    #[test]
    fn test_get_isa_cost_for_credits() {
        // formula: isa_needed = credits * credit_price_usd / isa_price_usd
        //          = credits * 100 / isa_price_usd
        // 5_000_000 credits at $0.50/ISA: 5_000_000 * 100 / 500_000 = 1_000 micro-ISA units
        let cost = CreditSystem::get_isa_cost_for_credits(5_000_000, ISA_PRICE);
        assert_eq!(cost, 1_000);

        // At $1.00/ISA: 10_000_000 credits * 100 / 1_000_000 = 1_000 micro-ISA units
        let cost2 = CreditSystem::get_isa_cost_for_credits(10_000_000, 1_000_000);
        assert_eq!(cost2, 1_000);
    }

    // ----------------------------------------------------------------
    // test_get_credits_for_isa
    // ----------------------------------------------------------------

    #[test]
    fn test_get_credits_for_isa() {
        // 1_000_000 micro-ISA at $0.50/ISA → 5_000 credits
        // credits = (1_000_000 * 500_000) / (100 * 1_000_000) = 5_000
        let credits = CreditSystem::credits_for_isa(1_000_000, ISA_PRICE);
        assert_eq!(credits, 5_000);

        // 2_000_000 micro-ISA at $1.00/ISA → 20_000 credits
        let credits2 = CreditSystem::credits_for_isa(2_000_000, 1_000_000);
        assert_eq!(credits2, 20_000);

        // Zero ISA → zero credits
        let credits3 = CreditSystem::credits_for_isa(0, ISA_PRICE);
        assert_eq!(credits3, 0);
    }

    // ----------------------------------------------------------------
    // test_set_credit_price
    // ----------------------------------------------------------------

    #[test]
    fn test_set_credit_price() {
        let mut cs = setup();
        assert_eq!(cs.credit_price_usd, DEFAULT_CREDIT_PRICE_USD);

        cs.set_credit_price(20_000, &admin()).unwrap();
        assert_eq!(cs.credit_price_usd, 20_000);
    }

    // ----------------------------------------------------------------
    // test_unauthorized_admin_fails
    // ----------------------------------------------------------------

    #[test]
    fn test_unauthorized_admin_fails() {
        let mut cs = setup();

        let result = cs.set_credit_price(20_000, &random_addr());
        assert_eq!(result, Err(CreditError::UnauthorizedAdmin(random_addr())));

        let result2 = cs.grant_credits(user(), 100, &random_addr(), 1);
        assert_eq!(result2, Err(CreditError::UnauthorizedAdmin(random_addr())));
    }

    // ----------------------------------------------------------------
    // test_grant_credits
    // ----------------------------------------------------------------

    #[test]
    fn test_grant_credits() {
        let mut cs = setup();

        cs.grant_credits(user(), 500, &admin(), 5).unwrap();

        assert_eq!(cs.get_balance(&user()), 500);
        assert_eq!(cs.total_credits_issued, 500);

        let acct = cs.get_account(&user()).unwrap();
        assert_eq!(acct.last_top_up_height, 5);
    }

    // ----------------------------------------------------------------
    // test_total_credits_tracking
    // ----------------------------------------------------------------

    #[test]
    fn test_total_credits_tracking() {
        let mut cs = setup();

        // Issue 10_000 credits via purchase
        cs.purchase_credits(user(), 2_000_000, ISA_PRICE, 1)
            .unwrap(); // 10_000 credits

        // Grant 50 more
        cs.grant_credits(user2(), 50, &admin(), 2).unwrap();

        assert_eq!(cs.total_credits_issued, 10_050);
        assert_eq!(cs.total_credits_burned, 0);
        assert_eq!(cs.total_credits_in_circulation(), 10_050);

        // Spend 30
        cs.spend_credits(&user(), 30).unwrap();
        assert_eq!(cs.total_credits_burned, 30);
        assert_eq!(cs.total_credits_in_circulation(), 10_020);
    }

    // ----------------------------------------------------------------
    // test_multiple_purchases
    // ----------------------------------------------------------------

    #[test]
    fn test_multiple_purchases() {
        let mut cs = setup();

        // First purchase: 2 ISA → 10_000 credits
        cs.purchase_credits(user(), 2_000_000, ISA_PRICE, 10)
            .unwrap();

        // Second purchase: 4 ISA → 20_000 credits
        cs.purchase_credits(user(), 4_000_000, ISA_PRICE, 20)
            .unwrap();

        let acct = cs.get_account(&user()).unwrap();
        assert_eq!(acct.credit_balance, 30_000);
        assert_eq!(acct.total_credits_purchased, 30_000);
        assert_eq!(acct.last_top_up_height, 20);
        assert_eq!(cs.total_credits_issued, 30_000);
    }

    // ----------------------------------------------------------------
    // test_spend_account_not_found
    // ----------------------------------------------------------------

    #[test]
    fn test_spend_account_not_found() {
        let mut cs = setup();
        let result = cs.spend_credits(&random_addr(), 10);
        assert_eq!(result, Err(CreditError::AccountNotFound(random_addr())));
    }

    // ----------------------------------------------------------------
    // test_invalid_amount_purchase
    // ----------------------------------------------------------------

    #[test]
    fn test_invalid_amount_purchase() {
        let mut cs = setup();
        let result = cs.purchase_credits(user(), 0, ISA_PRICE, 1);
        assert_eq!(result, Err(CreditError::InvalidAmount));
    }
}
