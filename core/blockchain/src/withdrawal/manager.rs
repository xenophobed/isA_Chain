use super::types::{
    WithdrawalError, WithdrawalRequest, WithdrawalStatus, BPS_DENOMINATOR,
};
use crate::types::{Address, Amount, BlockHeight, Hash};
use std::collections::HashMap;

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
