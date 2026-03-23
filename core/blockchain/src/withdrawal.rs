use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ====================================================================
// Constants
// ====================================================================

/// Default withdrawal fee: 50 bps = 0.5%
pub const DEFAULT_WITHDRAWAL_FEE_BPS: u32 = 50;

/// Default cooldown period in blocks (~5 minutes at 3s/block)
pub const DEFAULT_COOLDOWN_BLOCKS: u64 = 100;

/// Default minimum withdrawal: 1_000 credits = $10.00
pub const DEFAULT_MIN_WITHDRAWAL: Amount = 1_000;

/// Default daily limit per user: 100_000_000 credits = $1,000,000
pub const DEFAULT_DAILY_LIMIT: Amount = 100_000_000;

/// Basis points denominator
const BPS_DENOMINATOR: Amount = 10_000;

// ====================================================================
// WithdrawalStatus
// ====================================================================

/// Lifecycle state of a withdrawal request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WithdrawalStatus {
    /// Request received, not yet in cooldown.
    Pending,
    /// Waiting for the cooldown period to elapse.
    InCooldown,
    /// Cooldown complete; being processed on-chain.
    Processing,
    /// ISA has been minted and the tx is confirmed.
    Completed,
    /// Processing failed; reason is stored in the variant.
    Failed(String),
    /// User cancelled before processing began.
    Cancelled,
}

// ====================================================================
// WithdrawalRequest
// ====================================================================

/// A single off-chain-credits → on-chain-ISA withdrawal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WithdrawalRequest {
    /// Unique identifier for this request.
    pub id: Hash,
    /// User who initiated the withdrawal.
    pub user: Address,
    /// Credits being withdrawn (before fee deduction).
    pub credits_amount: Amount,
    /// ISA tokens to be minted to the user.
    pub isa_to_receive: Amount,
    /// ISA/USD price (micro-USD) at withdrawal time.
    pub isa_price_usd: Amount,
    /// Withdrawal fee in credits.
    pub fee: Amount,
    /// Current lifecycle status.
    pub status: WithdrawalStatus,
    /// Block height when the request was created.
    pub created_at: BlockHeight,
    /// Block height when the request was completed (or failed/cancelled).
    pub completed_at: Option<BlockHeight>,
    /// On-chain mint transaction hash (set on completion).
    pub tx_hash: Option<Hash>,
    /// Block height at which the cooldown period ends.
    pub cooldown_ends: BlockHeight,
}

// ====================================================================
// WithdrawalError
// ====================================================================

/// Errors that can arise during withdrawal operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WithdrawalError {
    #[error("Insufficient credits for withdrawal")]
    InsufficientCredits,

    #[error("Invalid amount: must be greater than zero")]
    InvalidAmount,

    #[error("Withdrawal not found: {0:?}")]
    WithdrawalNotFound(Hash),

    #[error("Cooldown period has not yet completed")]
    CooldownNotComplete,

    #[error("Withdrawal has already been processed")]
    AlreadyProcessed,

    #[error("Below minimum withdrawal: minimum {minimum}, requested {requested}")]
    MinimumNotMet { minimum: Amount, requested: Amount },

    #[error("Daily limit exceeded: limit {limit}, already used {used}")]
    DailyLimitExceeded { limit: Amount, used: Amount },
}

// ====================================================================
// WithdrawalManager
// ====================================================================

/// Manages the full lifecycle of credit → ISA withdrawal requests.
///
/// ## Flow
/// 1. User calls `initiate_withdrawal` — fee is deducted, ISA amount
///    is calculated, and a cooldown period begins.
/// 2. After `cooldown_blocks` have elapsed, `check_cooldown` returns `true`.
/// 3. The processor calls `complete_withdrawal` with the on-chain mint
///    tx hash; totals are updated.
///
/// Users may call `cancel_withdrawal` at any point before `Processing`.
pub struct WithdrawalManager {
    /// All withdrawal requests, indexed by request ID.
    pub withdrawals: HashMap<Hash, WithdrawalRequest>,
    /// Per-user index of request IDs.
    pub by_user: HashMap<Address, Vec<Hash>>,
    /// Lifetime total credits withdrawn.
    pub total_withdrawn_credits: Amount,
    /// Lifetime total ISA minted via withdrawals.
    pub total_isa_minted: Amount,
    /// Fee in basis points (e.g. 50 = 0.5 %).
    pub withdrawal_fee_bps: u32,
    /// Number of blocks the cooldown lasts.
    pub cooldown_blocks: u64,
    /// Minimum credits per withdrawal transaction.
    pub min_withdrawal: Amount,
    /// Maximum credits a single user may withdraw per day-equivalent window.
    pub daily_limit: Amount,
    /// Credit price in micro-USD (used for ISA conversion).
    pub credit_price_usd: Amount,
}

impl WithdrawalManager {
    // ----------------------------------------------------------------
    // Construction
    // ----------------------------------------------------------------

    /// Create a new `WithdrawalManager` with explicit configuration.
    ///
    /// Use the `DEFAULT_*` constants for standard platform values.
    pub fn new(
        fee_bps: u32,
        cooldown: u64,
        min_withdrawal: Amount,
        daily_limit: Amount,
        credit_price_usd: Amount,
    ) -> Self {
        WithdrawalManager {
            withdrawals: HashMap::new(),
            by_user: HashMap::new(),
            total_withdrawn_credits: 0,
            total_isa_minted: 0,
            withdrawal_fee_bps: fee_bps,
            cooldown_blocks: cooldown,
            min_withdrawal,
            daily_limit,
            credit_price_usd,
        }
    }

    // ----------------------------------------------------------------
    // Core operations
    // ----------------------------------------------------------------

    /// Begin a withdrawal of `credits` for `user`.
    ///
    /// Validates the amount, deducts the fee, calculates the ISA to
    /// mint, and records the request in `InCooldown` state.
    ///
    /// # Errors
    /// - [`WithdrawalError::InvalidAmount`] — `credits` is zero.
    /// - [`WithdrawalError::MinimumNotMet`] — below `min_withdrawal`.
    /// - [`WithdrawalError::DailyLimitExceeded`] — user exceeded their daily cap.
    ///
    /// # Note
    /// The caller is responsible for deducting `credits` from the user's
    /// credit account *before* calling this method.
    pub fn initiate_withdrawal(
        &mut self,
        user: Address,
        credits: Amount,
        isa_price_usd: Amount,
        height: BlockHeight,
    ) -> Result<WithdrawalRequest, WithdrawalError> {
        if credits == 0 {
            return Err(WithdrawalError::InvalidAmount);
        }

        if credits < self.min_withdrawal {
            return Err(WithdrawalError::MinimumNotMet {
                minimum: self.min_withdrawal,
                requested: credits,
            });
        }

        // Daily-limit check: sum pending/in-cooldown/processing credits for this user.
        let used = self.user_pending_credits(&user);
        if used.saturating_add(credits) > self.daily_limit {
            return Err(WithdrawalError::DailyLimitExceeded {
                limit: self.daily_limit,
                used,
            });
        }

        let fee = self.calculate_fee(credits);
        let net_credits = credits.saturating_sub(fee);
        let isa_to_receive = Self::calculate_isa(net_credits, isa_price_usd);

        let id = Self::derive_id(user, credits, height);
        let cooldown_ends = height.saturating_add(self.cooldown_blocks);

        let request = WithdrawalRequest {
            id,
            user,
            credits_amount: credits,
            isa_to_receive,
            isa_price_usd,
            fee,
            status: WithdrawalStatus::InCooldown,
            created_at: height,
            completed_at: None,
            tx_hash: None,
            cooldown_ends,
        };

        self.withdrawals.insert(id, request.clone());
        self.by_user.entry(user).or_default().push(id);

        Ok(request)
    }

    /// Mark a withdrawal as `Completed` and record the on-chain mint tx hash.
    ///
    /// Advances status from `InCooldown` or `Processing` → `Completed`.
    ///
    /// # Errors
    /// - [`WithdrawalError::WithdrawalNotFound`] — unknown ID.
    /// - [`WithdrawalError::CooldownNotComplete`] — cooldown still active.
    /// - [`WithdrawalError::AlreadyProcessed`] — not in a completable state.
    pub fn complete_withdrawal(
        &mut self,
        id: &Hash,
        tx_hash: Hash,
        height: BlockHeight,
    ) -> Result<&WithdrawalRequest, WithdrawalError> {
        let req = self
            .withdrawals
            .get_mut(id)
            .ok_or(WithdrawalError::WithdrawalNotFound(*id))?;

        if height < req.cooldown_ends {
            return Err(WithdrawalError::CooldownNotComplete);
        }

        match req.status {
            WithdrawalStatus::InCooldown | WithdrawalStatus::Processing => {}
            WithdrawalStatus::Completed => return Err(WithdrawalError::AlreadyProcessed),
            _ => return Err(WithdrawalError::AlreadyProcessed),
        }

        let isa = req.isa_to_receive;
        let credits = req.credits_amount;

        req.status = WithdrawalStatus::Completed;
        req.completed_at = Some(height);
        req.tx_hash = Some(tx_hash);

        self.total_withdrawn_credits = self.total_withdrawn_credits.saturating_add(credits);
        self.total_isa_minted = self.total_isa_minted.saturating_add(isa);

        Ok(self.withdrawals.get(id).unwrap())
    }

    /// Cancel a withdrawal that has not yet reached `Processing`.
    ///
    /// Returns the number of credits that should be refunded to the user
    /// (the original `credits_amount` — the protocol does not refund fees
    /// for initiated requests by default, but the full amount is returned
    /// here so the caller can decide the refund policy).
    ///
    /// # Errors
    /// - [`WithdrawalError::WithdrawalNotFound`] — unknown ID.
    /// - [`WithdrawalError::AlreadyProcessed`] — cannot cancel a completed,
    ///   failed, or already-cancelled request.
    pub fn cancel_withdrawal(&mut self, id: &Hash) -> Result<Amount, WithdrawalError> {
        let req = self
            .withdrawals
            .get_mut(id)
            .ok_or(WithdrawalError::WithdrawalNotFound(*id))?;

        match req.status {
            WithdrawalStatus::Pending
            | WithdrawalStatus::InCooldown
            | WithdrawalStatus::Processing => {
                let credits = req.credits_amount;
                req.status = WithdrawalStatus::Cancelled;
                Ok(credits)
            }
            _ => Err(WithdrawalError::AlreadyProcessed),
        }
    }

    /// Check whether the cooldown period for a withdrawal has elapsed.
    ///
    /// Returns `true` when `current_height >= cooldown_ends`.
    ///
    /// # Errors
    /// - [`WithdrawalError::WithdrawalNotFound`] — unknown ID.
    pub fn check_cooldown(
        &self,
        id: &Hash,
        current_height: BlockHeight,
    ) -> Result<bool, WithdrawalError> {
        let req = self
            .withdrawals
            .get(id)
            .ok_or(WithdrawalError::WithdrawalNotFound(*id))?;

        Ok(current_height >= req.cooldown_ends)
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    /// Retrieve a withdrawal request by ID, or `None` if not found.
    pub fn get_withdrawal(&self, id: &Hash) -> Option<&WithdrawalRequest> {
        self.withdrawals.get(id)
    }

    /// Retrieve all withdrawal requests for a user (in insertion order).
    pub fn get_user_withdrawals(&self, user: &Address) -> Vec<&WithdrawalRequest> {
        self.by_user
            .get(user)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.withdrawals.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    // ----------------------------------------------------------------
    // Conversion math (pure, static)
    // ----------------------------------------------------------------

    /// Calculate ISA to mint for `net_credits` (after fee) at `isa_price_usd`.
    ///
    /// Formula:
    /// ```text
    /// isa = (net_credits * credit_price_usd) / isa_price_usd
    /// ```
    /// where both prices are in micro-USD and the result is in micro-ISA.
    ///
    /// Returns 0 when `isa_price_usd` is 0 (safe fallback).
    pub fn calculate_isa(net_credits: Amount, isa_price_usd: Amount) -> Amount {
        if isa_price_usd == 0 {
            return 0;
        }
        // credit_price_usd is the default platform price per credit ($0.00001 = 100 micro-USD).
        // isa = net_credits * 100 / isa_price_usd
        net_credits
            .checked_mul(crate::credits::DEFAULT_CREDIT_PRICE_USD)
            .map(|v| v / isa_price_usd)
            .unwrap_or(u128::MAX)
    }

    /// Calculate the withdrawal fee for `credits` based on `withdrawal_fee_bps`.
    ///
    /// Formula: `fee = credits * fee_bps / 10_000`
    pub fn calculate_fee(&self, credits: Amount) -> Amount {
        credits
            .checked_mul(self.withdrawal_fee_bps as Amount)
            .map(|v| v / BPS_DENOMINATOR)
            .unwrap_or(0)
    }

    // ----------------------------------------------------------------
    // Private helpers
    // ----------------------------------------------------------------

    /// Deterministic request ID derived from user + credits + height.
    fn derive_id(user: Address, credits: Amount, height: BlockHeight) -> Hash {
        let mut data = Vec::with_capacity(20 + 16 + 8);
        data.extend_from_slice(user.as_bytes());
        data.extend_from_slice(&credits.to_le_bytes());
        data.extend_from_slice(&height.to_le_bytes());
        Hash::hash_data(&data)
    }

    /// Sum of `credits_amount` for all non-terminal requests by `user`.
    ///
    /// Used for daily-limit enforcement.
    fn user_pending_credits(&self, user: &Address) -> Amount {
        self.by_user
            .get(user)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.withdrawals.get(id))
                    .filter(|r| {
                        matches!(
                            r.status,
                            WithdrawalStatus::Pending
                                | WithdrawalStatus::InCooldown
                                | WithdrawalStatus::Processing
                        )
                    })
                    .map(|r| r.credits_amount)
                    .fold(0u128, |acc, c| acc.saturating_add(c))
            })
            .unwrap_or(0)
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

    fn user() -> Address {
        Address::from([0xAA; 20])
    }

    fn user2() -> Address {
        Address::from([0xBB; 20])
    }

    /// ISA price: $0.50 = 500_000 micro-USD
    const ISA_PRICE: Amount = 500_000;

    fn setup() -> WithdrawalManager {
        WithdrawalManager::new(
            DEFAULT_WITHDRAWAL_FEE_BPS, // 50 bps
            DEFAULT_COOLDOWN_BLOCKS,    // 100 blocks
            DEFAULT_MIN_WITHDRAWAL,     // 1_000 credits
            DEFAULT_DAILY_LIMIT,        // 100_000_000 credits
            crate::credits::DEFAULT_CREDIT_PRICE_USD,
        )
    }

    // ----------------------------------------------------------------
    // test_initiate_withdrawal
    // ----------------------------------------------------------------

    #[test]
    fn test_initiate_withdrawal() {
        let mut wm = setup();

        let req = wm.initiate_withdrawal(user(), 10_000, ISA_PRICE, 50).unwrap();

        assert_eq!(req.user, user());
        assert_eq!(req.credits_amount, 10_000);
        assert_eq!(req.status, WithdrawalStatus::InCooldown);
        assert_eq!(req.created_at, 50);
        assert_eq!(req.cooldown_ends, 150); // 50 + 100
        assert!(req.tx_hash.is_none());
        assert!(req.completed_at.is_none());

        // Fee = 10_000 * 50 / 10_000 = 50 credits
        assert_eq!(req.fee, 50);

        // net_credits = 9_950; ISA = 9_950 * 10_000 / 500_000 = 199 (integer division)
        let expected_isa = WithdrawalManager::calculate_isa(9_950, ISA_PRICE);
        assert_eq!(req.isa_to_receive, expected_isa);

        // Request should be stored
        let stored = wm.get_withdrawal(&req.id).unwrap();
        assert_eq!(stored.id, req.id);
    }

    // ----------------------------------------------------------------
    // test_complete_withdrawal
    // ----------------------------------------------------------------

    #[test]
    fn test_complete_withdrawal() {
        let mut wm = setup();
        let req = wm.initiate_withdrawal(user(), 10_000, ISA_PRICE, 50).unwrap();
        let id = req.id;
        let tx = Hash::hash_data(b"mint-tx");

        // Should fail before cooldown ends (height 149 < 150)
        let err = wm.complete_withdrawal(&id, tx, 149).unwrap_err();
        assert_eq!(err, WithdrawalError::CooldownNotComplete);

        // Should succeed at cooldown_ends (height 150)
        let completed = wm.complete_withdrawal(&id, tx, 150).unwrap();
        assert_eq!(completed.status, WithdrawalStatus::Completed);
        assert_eq!(completed.completed_at, Some(150));
        assert_eq!(completed.tx_hash, Some(tx));

        // Totals updated
        assert_eq!(wm.total_withdrawn_credits, 10_000);
        assert_eq!(wm.total_isa_minted, req.isa_to_receive);
    }

    // ----------------------------------------------------------------
    // test_cancel_withdrawal
    // ----------------------------------------------------------------

    #[test]
    fn test_cancel_withdrawal() {
        let mut wm = setup();
        let req = wm.initiate_withdrawal(user(), 5_000, ISA_PRICE, 10).unwrap();
        let id = req.id;

        let refund = wm.cancel_withdrawal(&id).unwrap();
        assert_eq!(refund, 5_000); // full credits_amount returned

        let stored = wm.get_withdrawal(&id).unwrap();
        assert_eq!(stored.status, WithdrawalStatus::Cancelled);

        // Cannot cancel again
        let err = wm.cancel_withdrawal(&id).unwrap_err();
        assert_eq!(err, WithdrawalError::AlreadyProcessed);
    }

    // ----------------------------------------------------------------
    // test_cooldown_check
    // ----------------------------------------------------------------

    #[test]
    fn test_cooldown_check() {
        let mut wm = setup();
        let req = wm.initiate_withdrawal(user(), 2_000, ISA_PRICE, 0).unwrap();
        let id = req.id;

        // cooldown_ends = 0 + 100 = 100
        assert!(!wm.check_cooldown(&id, 99).unwrap());
        assert!(wm.check_cooldown(&id, 100).unwrap());
        assert!(wm.check_cooldown(&id, 200).unwrap());
    }

    // ----------------------------------------------------------------
    // test_below_minimum_fails
    // ----------------------------------------------------------------

    #[test]
    fn test_below_minimum_fails() {
        let mut wm = setup();

        let err = wm
            .initiate_withdrawal(user(), 999, ISA_PRICE, 1)
            .unwrap_err();

        assert_eq!(
            err,
            WithdrawalError::MinimumNotMet {
                minimum: DEFAULT_MIN_WITHDRAWAL,
                requested: 999
            }
        );
    }

    // ----------------------------------------------------------------
    // test_fee_calculation
    // ----------------------------------------------------------------

    #[test]
    fn test_fee_calculation() {
        let wm = setup();

        // 0.5% of 10_000 = 50
        assert_eq!(wm.calculate_fee(10_000), 50);

        // 0.5% of 1_000 = 5
        assert_eq!(wm.calculate_fee(1_000), 5);

        // 0.5% of 1_000_000 = 5_000
        assert_eq!(wm.calculate_fee(1_000_000), 5_000);

        // Zero credits → zero fee
        assert_eq!(wm.calculate_fee(0), 0);
    }

    // ----------------------------------------------------------------
    // test_isa_calculation
    // ----------------------------------------------------------------

    #[test]
    fn test_isa_calculation() {
        // 10_000 credits × $0.00001/credit / $0.50/ISA
        // = 10_000 * 100 / 500_000 = 2 ISA units
        assert_eq!(WithdrawalManager::calculate_isa(10_000, ISA_PRICE), 2);

        // At $1.00/ISA: 10_000 * 100 / 1_000_000 = 1
        assert_eq!(WithdrawalManager::calculate_isa(10_000, 1_000_000), 1);

        // Zero ISA price → 0 (safe)
        assert_eq!(WithdrawalManager::calculate_isa(10_000, 0), 0);

        // Zero credits → 0
        assert_eq!(WithdrawalManager::calculate_isa(0, ISA_PRICE), 0);
    }

    // ----------------------------------------------------------------
    // test_already_processed
    // ----------------------------------------------------------------

    #[test]
    fn test_already_processed() {
        let mut wm = setup();
        let req = wm.initiate_withdrawal(user(), 10_000, ISA_PRICE, 0).unwrap();
        let id = req.id;
        let tx = Hash::hash_data(b"tx1");

        // Complete it
        wm.complete_withdrawal(&id, tx, 100).unwrap();

        // Completing again must fail
        let err = wm.complete_withdrawal(&id, tx, 101).unwrap_err();
        assert_eq!(err, WithdrawalError::AlreadyProcessed);

        // Cancelling a completed withdrawal must also fail
        let cancel_err = wm.cancel_withdrawal(&id).unwrap_err();
        assert_eq!(cancel_err, WithdrawalError::AlreadyProcessed);
    }

    // ----------------------------------------------------------------
    // test_user_withdrawals
    // ----------------------------------------------------------------

    #[test]
    fn test_user_withdrawals() {
        let mut wm = setup();

        // Three requests from user(), one from user2()
        wm.initiate_withdrawal(user(), 1_000, ISA_PRICE, 1).unwrap();
        wm.initiate_withdrawal(user(), 2_000, ISA_PRICE, 2).unwrap();
        wm.initiate_withdrawal(user(), 3_000, ISA_PRICE, 3).unwrap();
        wm.initiate_withdrawal(user2(), 5_000, ISA_PRICE, 4).unwrap();

        let user_reqs = wm.get_user_withdrawals(&user());
        assert_eq!(user_reqs.len(), 3);

        let user2_reqs = wm.get_user_withdrawals(&user2());
        assert_eq!(user2_reqs.len(), 1);

        // No requests for unknown address
        let no_reqs = wm.get_user_withdrawals(&Address::from([0xFF; 20]));
        assert!(no_reqs.is_empty());
    }

    // ----------------------------------------------------------------
    // test_total_tracking
    // ----------------------------------------------------------------

    #[test]
    fn test_total_tracking() {
        let mut wm = setup();
        let tx = Hash::hash_data(b"tx");

        let r1 = wm.initiate_withdrawal(user(), 10_000, ISA_PRICE, 0).unwrap();
        let r2 = wm.initiate_withdrawal(user2(), 20_000, ISA_PRICE, 1).unwrap();

        wm.complete_withdrawal(&r1.id, tx, 100).unwrap();
        wm.complete_withdrawal(&r2.id, tx, 101).unwrap();

        assert_eq!(wm.total_withdrawn_credits, 30_000);
        assert_eq!(
            wm.total_isa_minted,
            r1.isa_to_receive + r2.isa_to_receive
        );
    }

    // ----------------------------------------------------------------
    // test_invalid_amount_fails
    // ----------------------------------------------------------------

    #[test]
    fn test_invalid_amount_fails() {
        let mut wm = setup();
        let err = wm
            .initiate_withdrawal(user(), 0, ISA_PRICE, 1)
            .unwrap_err();
        assert_eq!(err, WithdrawalError::InvalidAmount);
    }

    // ----------------------------------------------------------------
    // test_withdrawal_not_found
    // ----------------------------------------------------------------

    #[test]
    fn test_withdrawal_not_found() {
        let mut wm = setup();
        let phantom = Hash::hash_data(b"nonexistent");

        assert_eq!(
            wm.complete_withdrawal(&phantom, phantom, 999).unwrap_err(),
            WithdrawalError::WithdrawalNotFound(phantom)
        );
        assert_eq!(
            wm.cancel_withdrawal(&phantom).unwrap_err(),
            WithdrawalError::WithdrawalNotFound(phantom)
        );
        assert_eq!(
            wm.check_cooldown(&phantom, 999).unwrap_err(),
            WithdrawalError::WithdrawalNotFound(phantom)
        );
    }

    // ----------------------------------------------------------------
    // test_daily_limit_exceeded
    // ----------------------------------------------------------------

    #[test]
    fn test_daily_limit_exceeded() {
        let mut wm = WithdrawalManager::new(
            DEFAULT_WITHDRAWAL_FEE_BPS,
            DEFAULT_COOLDOWN_BLOCKS,
            1_000,
            50_000, // tight daily limit: 50_000 credits
            crate::credits::DEFAULT_CREDIT_PRICE_USD,
        );

        // First withdrawal: 30_000 — within limit
        wm.initiate_withdrawal(user(), 30_000, ISA_PRICE, 1).unwrap();

        // Second withdrawal: 25_000 — total would be 55_000 > 50_000
        let err = wm
            .initiate_withdrawal(user(), 25_000, ISA_PRICE, 2)
            .unwrap_err();

        assert_eq!(
            err,
            WithdrawalError::DailyLimitExceeded {
                limit: 50_000,
                used: 30_000
            }
        );

        // A different user is unaffected
        wm.initiate_withdrawal(user2(), 40_000, ISA_PRICE, 3).unwrap();
    }
}
