use crate::types::{Address, Amount, BlockHeight, Hash};
use std::collections::HashMap;

use super::types::{DepositError, DepositRequest, DepositStatus};

const MICRO_USD_SCALE: Amount = 1_000_000;

// ====================================================================
// DepositManager
// ====================================================================

/// Manages the full lifecycle of on-chain ISA → off-chain credit deposits.
///
/// ## Conversion formula
/// ```text
/// credits = (isa_amount * isa_price_usd) / (credit_price_usd * MICRO_USD_SCALE)
/// ```
/// Example: 2_000_000 micro-ISA × 500_000 micro-USD/ISA / (10_000 × 1_000_000) = 100 credits
pub struct DepositManager {
    /// All deposit requests, keyed by deposit ID.
    pub deposits: HashMap<Hash, DepositRequest>,
    /// Deposit IDs grouped by user address.
    pub by_user: HashMap<Address, Vec<Hash>>,
    /// Cumulative ISA deposited across all completed deposits (micro-ISA).
    pub total_deposited_isa: Amount,
    /// Cumulative credits issued across all completed deposits.
    pub total_credits_issued: Amount,
    /// Price per credit in micro-USD (default 10_000 = $0.01).
    pub credit_price_usd: Amount,
}

impl DepositManager {
    // ----------------------------------------------------------------
    // Construction
    // ----------------------------------------------------------------

    /// Create a new `DepositManager`.
    ///
    /// - `credit_price_usd`: price per credit in micro-USD
    ///   (use [`crate::deposit::DEPOSIT_DEFAULT_CREDIT_PRICE_USD`] for $0.01).
    pub fn new(credit_price_usd: Amount) -> Self {
        DepositManager {
            deposits: HashMap::new(),
            by_user: HashMap::new(),
            total_deposited_isa: 0,
            total_credits_issued: 0,
            credit_price_usd,
        }
    }

    // ----------------------------------------------------------------
    // Core operations
    // ----------------------------------------------------------------

    /// Initiate a new deposit.
    ///
    /// Validates inputs, calculates the credits the user will receive, and
    /// records the deposit in `Pending` state.
    ///
    /// Fails with:
    /// - [`DepositError::InvalidAmount`] if `isa_amount` is zero.
    /// - [`DepositError::PriceStale`] if `isa_price_usd` is zero.
    pub fn initiate_deposit(
        &mut self,
        user: Address,
        isa_amount: Amount,
        isa_price_usd: Amount,
        height: BlockHeight,
    ) -> Result<DepositRequest, DepositError> {
        if isa_amount == 0 {
            return Err(DepositError::InvalidAmount);
        }
        if isa_price_usd == 0 {
            return Err(DepositError::PriceStale);
        }

        let credits_to_receive =
            Self::calculate_credits_inner(isa_amount, isa_price_usd, self.credit_price_usd);

        // Derive a deterministic deposit ID from the key fields.
        let id = Self::derive_deposit_id(user, isa_amount, height);

        let deposit = DepositRequest {
            id,
            user,
            isa_amount,
            credits_to_receive,
            isa_price_usd,
            status: DepositStatus::Pending,
            created_at: height,
            completed_at: None,
            tx_hash: None,
        };

        self.deposits.insert(id, deposit.clone());
        self.by_user.entry(user).or_default().push(id);

        Ok(deposit)
    }

    /// Mark a deposit as completed after the on-chain burn is confirmed.
    ///
    /// Transitions: `Pending | Processing` → `Completed`.
    /// Updates running totals.
    ///
    /// Fails with:
    /// - [`DepositError::DepositNotFound`] if the ID is unknown.
    /// - [`DepositError::AlreadyProcessed`] if already in a terminal state.
    pub fn complete_deposit(
        &mut self,
        deposit_id: &Hash,
        tx_hash: Hash,
        height: BlockHeight,
    ) -> Result<&DepositRequest, DepositError> {
        let deposit = self
            .deposits
            .get_mut(deposit_id)
            .ok_or(DepositError::DepositNotFound(*deposit_id))?;

        match &deposit.status {
            DepositStatus::Completed | DepositStatus::Refunded => {
                return Err(DepositError::AlreadyProcessed);
            }
            DepositStatus::Failed(_) => {
                return Err(DepositError::AlreadyProcessed);
            }
            _ => {}
        }

        let isa_amount = deposit.isa_amount;
        let credits = deposit.credits_to_receive;

        deposit.status = DepositStatus::Completed;
        deposit.tx_hash = Some(tx_hash);
        deposit.completed_at = Some(height);

        self.total_deposited_isa += isa_amount;
        self.total_credits_issued += credits;

        Ok(self.deposits.get(deposit_id).unwrap())
    }

    /// Mark a deposit as failed.
    ///
    /// Transitions: `Pending | Processing` → `Failed(reason)`.
    ///
    /// Fails with:
    /// - [`DepositError::DepositNotFound`] if the ID is unknown.
    /// - [`DepositError::AlreadyProcessed`] if already in a terminal state.
    pub fn fail_deposit(
        &mut self,
        deposit_id: &Hash,
        reason: String,
    ) -> Result<(), DepositError> {
        let deposit = self
            .deposits
            .get_mut(deposit_id)
            .ok_or(DepositError::DepositNotFound(*deposit_id))?;

        match &deposit.status {
            DepositStatus::Completed | DepositStatus::Refunded => {
                return Err(DepositError::AlreadyProcessed);
            }
            DepositStatus::Failed(_) => {
                return Err(DepositError::AlreadyProcessed);
            }
            _ => {}
        }

        deposit.status = DepositStatus::Failed(reason);
        Ok(())
    }

    /// Refund a failed deposit, returning the ISA amount to the caller.
    ///
    /// Transitions: `Failed(_)` → `Refunded`.
    /// Returns the `isa_amount` that should be returned to the user.
    ///
    /// Fails with:
    /// - [`DepositError::DepositNotFound`] if the ID is unknown.
    /// - [`DepositError::AlreadyProcessed`] if not in `Failed` state.
    pub fn refund_deposit(&mut self, deposit_id: &Hash) -> Result<Amount, DepositError> {
        let deposit = self
            .deposits
            .get_mut(deposit_id)
            .ok_or(DepositError::DepositNotFound(*deposit_id))?;

        match &deposit.status {
            DepositStatus::Failed(_) => {}
            _ => return Err(DepositError::AlreadyProcessed),
        }

        let isa_amount = deposit.isa_amount;
        deposit.status = DepositStatus::Refunded;
        Ok(isa_amount)
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    /// Look up a deposit by ID.
    pub fn get_deposit(&self, id: &Hash) -> Option<&DepositRequest> {
        self.deposits.get(id)
    }

    /// Return all deposits for a given user, in insertion order.
    pub fn get_user_deposits(&self, user: &Address) -> Vec<&DepositRequest> {
        self.by_user
            .get(user)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.deposits.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Total ISA deposited across all *completed* deposits (micro-ISA).
    pub fn get_total_deposited(&self) -> Amount {
        self.total_deposited_isa
    }

    /// Total credits issued across all *completed* deposits.
    pub fn get_total_credits_issued(&self) -> Amount {
        self.total_credits_issued
    }

    // ----------------------------------------------------------------
    // Conversion helper
    // ----------------------------------------------------------------

    /// Calculate how many credits `isa_amount` buys at `isa_price_usd`,
    /// using this manager's `credit_price_usd`.
    ///
    /// Formula: `credits = (isa_amount * isa_price_usd) / (credit_price_usd * MICRO_USD_SCALE)`
    pub fn calculate_credits(&self, isa_amount: Amount, isa_price_usd: Amount) -> Amount {
        Self::calculate_credits_inner(isa_amount, isa_price_usd, self.credit_price_usd)
    }

    // ----------------------------------------------------------------
    // Private helpers
    // ----------------------------------------------------------------

    fn calculate_credits_inner(
        isa_amount: Amount,
        isa_price_usd: Amount,
        credit_price_usd: Amount,
    ) -> Amount {
        if credit_price_usd == 0 || isa_price_usd == 0 {
            return 0;
        }
        let denominator = credit_price_usd.saturating_mul(MICRO_USD_SCALE);
        isa_amount
            .checked_mul(isa_price_usd)
            .map(|v| v / denominator)
            .unwrap_or(u128::MAX)
    }

    /// Derive a deterministic deposit ID from the user, amount, and block height.
    fn derive_deposit_id(user: Address, isa_amount: Amount, height: BlockHeight) -> Hash {
        let mut data = Vec::with_capacity(20 + 16 + 8);
        data.extend_from_slice(user.as_bytes());
        data.extend_from_slice(&isa_amount.to_le_bytes());
        data.extend_from_slice(&height.to_le_bytes());
        Hash::hash_data(&data)
    }
}
