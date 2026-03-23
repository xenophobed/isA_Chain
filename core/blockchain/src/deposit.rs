use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ====================================================================
// Constants
// ====================================================================

/// Default credit price: $0.00001 USD = 100 micro-USD
pub const DEPOSIT_DEFAULT_CREDIT_PRICE_USD: Amount = 100;

/// Micro-USD scale factor: 1_000_000 micro-USD = $1.00
const MICRO_USD_SCALE: Amount = 1_000_000;

// ====================================================================
// DepositStatus
// ====================================================================

/// Lifecycle status of a deposit request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DepositStatus {
    /// Deposit request created; on-chain burn not yet confirmed.
    Pending,
    /// On-chain burn detected; off-chain credit issuance in flight.
    Processing,
    /// Credits have been issued to the user.
    Completed,
    /// Deposit failed (reason attached).
    Failed(String),
    /// ISA was refunded to the user after a failed deposit.
    Refunded,
}

// ====================================================================
// DepositError
// ====================================================================

/// Errors that can occur during deposit operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DepositError {
    #[error("Insufficient balance to complete the deposit")]
    InsufficientBalance,

    #[error("Invalid amount: must be greater than zero")]
    InvalidAmount,

    #[error("Deposit not found: {0:?}")]
    DepositNotFound(Hash),

    #[error("Deposit has already been processed")]
    AlreadyProcessed,

    #[error("ISA price is stale or zero; cannot calculate credits")]
    PriceStale,
}

// ====================================================================
// DepositRequest
// ====================================================================

/// A record of a single on-chain → off-chain deposit.
///
/// Flow: `Pending` → `Processing` → `Completed`
///                              ↘ `Failed` → `Refunded`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepositRequest {
    /// Unique deposit ID (hash of user + isa_amount + created_at).
    pub id: Hash,
    /// User initiating the deposit.
    pub user: Address,
    /// ISA being deposited (micro-ISA units).
    pub isa_amount: Amount,
    /// Off-chain credits to be issued.
    pub credits_to_receive: Amount,
    /// ISA/USD price at time of deposit (micro-USD per ISA).
    pub isa_price_usd: Amount,
    /// Current lifecycle status.
    pub status: DepositStatus,
    /// Block height when deposit was initiated.
    pub created_at: BlockHeight,
    /// Block height when deposit reached a terminal state.
    pub completed_at: Option<BlockHeight>,
    /// On-chain burn transaction hash (set when Processing or later).
    pub tx_hash: Option<Hash>,
}

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
    ///   (use [`DEPOSIT_DEFAULT_CREDIT_PRICE_USD`] for $0.01).
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

    /// 2 ISA in micro-ISA (smallest unit)
    const TWO_ISA: Amount = 2_000_000;

    fn setup() -> DepositManager {
        DepositManager::new(DEPOSIT_DEFAULT_CREDIT_PRICE_USD)
    }

    // ----------------------------------------------------------------
    // test_initiate_deposit
    // ----------------------------------------------------------------

    #[test]
    fn test_initiate_deposit() {
        let mut dm = setup();

        let deposit = dm
            .initiate_deposit(user(), TWO_ISA, ISA_PRICE, 10)
            .unwrap();

        assert_eq!(deposit.user, user());
        assert_eq!(deposit.isa_amount, TWO_ISA);
        assert_eq!(deposit.isa_price_usd, ISA_PRICE);
        assert_eq!(deposit.status, DepositStatus::Pending);
        assert_eq!(deposit.created_at, 10);
        assert!(deposit.completed_at.is_none());
        assert!(deposit.tx_hash.is_none());

        // credits = (2_000_000 * 500_000) / (100 * 1_000_000) = 10_000
        assert_eq!(deposit.credits_to_receive, 10_000);

        // Should be stored
        assert!(dm.get_deposit(&deposit.id).is_some());
    }

    // ----------------------------------------------------------------
    // test_complete_deposit
    // ----------------------------------------------------------------

    #[test]
    fn test_complete_deposit() {
        let mut dm = setup();

        let deposit = dm
            .initiate_deposit(user(), TWO_ISA, ISA_PRICE, 10)
            .unwrap();
        let id = deposit.id;

        let tx = Hash::hash_data(b"burn-tx");
        let completed = dm.complete_deposit(&id, tx, 12).unwrap();

        assert_eq!(completed.status, DepositStatus::Completed);
        assert_eq!(completed.tx_hash, Some(tx));
        assert_eq!(completed.completed_at, Some(12));

        // Totals updated
        assert_eq!(dm.get_total_deposited(), TWO_ISA);
        assert_eq!(dm.get_total_credits_issued(), 10_000);
    }

    // ----------------------------------------------------------------
    // test_fail_deposit
    // ----------------------------------------------------------------

    #[test]
    fn test_fail_deposit() {
        let mut dm = setup();

        let deposit = dm
            .initiate_deposit(user(), TWO_ISA, ISA_PRICE, 10)
            .unwrap();
        let id = deposit.id;

        dm.fail_deposit(&id, "network error".to_string()).unwrap();

        let stored = dm.get_deposit(&id).unwrap();
        assert_eq!(
            stored.status,
            DepositStatus::Failed("network error".to_string())
        );

        // Totals unchanged
        assert_eq!(dm.get_total_deposited(), 0);
        assert_eq!(dm.get_total_credits_issued(), 0);
    }

    // ----------------------------------------------------------------
    // test_refund_deposit
    // ----------------------------------------------------------------

    #[test]
    fn test_refund_deposit() {
        let mut dm = setup();

        let deposit = dm
            .initiate_deposit(user(), TWO_ISA, ISA_PRICE, 10)
            .unwrap();
        let id = deposit.id;

        dm.fail_deposit(&id, "timeout".to_string()).unwrap();
        let refund_amount = dm.refund_deposit(&id).unwrap();

        assert_eq!(refund_amount, TWO_ISA);
        assert_eq!(
            dm.get_deposit(&id).unwrap().status,
            DepositStatus::Refunded
        );
    }

    // ----------------------------------------------------------------
    // test_zero_amount_fails
    // ----------------------------------------------------------------

    #[test]
    fn test_zero_amount_fails() {
        let mut dm = setup();

        let result = dm.initiate_deposit(user(), 0, ISA_PRICE, 1);
        assert_eq!(result, Err(DepositError::InvalidAmount));
    }

    // ----------------------------------------------------------------
    // test_zero_price_fails (PriceStale)
    // ----------------------------------------------------------------

    #[test]
    fn test_zero_price_fails() {
        let mut dm = setup();

        let result = dm.initiate_deposit(user(), TWO_ISA, 0, 1);
        assert_eq!(result, Err(DepositError::PriceStale));
    }

    // ----------------------------------------------------------------
    // test_credit_calculation
    // ----------------------------------------------------------------

    #[test]
    fn test_credit_calculation() {
        let dm = setup();

        // 2 ISA @ $0.50 → 10_000 credits
        // (2_000_000 * 500_000) / (100 * 1_000_000) = 10_000
        assert_eq!(dm.calculate_credits(2_000_000, 500_000), 10_000);

        // 10 ISA @ $1.00 → 100_000 credits
        assert_eq!(dm.calculate_credits(10_000_000, 1_000_000), 100_000);

        // 1 ISA @ $0.50 → 5_000 credits
        assert_eq!(dm.calculate_credits(1_000_000, 500_000), 5_000);

        // 0 ISA → 0 credits
        assert_eq!(dm.calculate_credits(0, 500_000), 0);
    }

    // ----------------------------------------------------------------
    // test_user_deposits
    // ----------------------------------------------------------------

    #[test]
    fn test_user_deposits() {
        let mut dm = setup();

        // user1 initiates two deposits at different heights
        dm.initiate_deposit(user(), TWO_ISA, ISA_PRICE, 10).unwrap();
        dm.initiate_deposit(user(), 4_000_000, ISA_PRICE, 11)
            .unwrap();

        // user2 initiates one deposit
        dm.initiate_deposit(user2(), TWO_ISA, ISA_PRICE, 12)
            .unwrap();

        assert_eq!(dm.get_user_deposits(&user()).len(), 2);
        assert_eq!(dm.get_user_deposits(&user2()).len(), 1);

        // Unknown user returns empty vec
        let unknown = Address::from([0xFF; 20]);
        assert!(dm.get_user_deposits(&unknown).is_empty());
    }

    // ----------------------------------------------------------------
    // test_total_tracking
    // ----------------------------------------------------------------

    #[test]
    fn test_total_tracking() {
        let mut dm = setup();

        let d1 = dm
            .initiate_deposit(user(), TWO_ISA, ISA_PRICE, 10)
            .unwrap();
        let d2 = dm
            .initiate_deposit(user2(), 4_000_000, ISA_PRICE, 11)
            .unwrap();

        // Complete first deposit (10_000 credits, 2_000_000 ISA)
        dm.complete_deposit(&d1.id, Hash::hash_data(b"tx1"), 12)
            .unwrap();
        assert_eq!(dm.get_total_deposited(), TWO_ISA);
        assert_eq!(dm.get_total_credits_issued(), 10_000);

        // Complete second deposit (20_000 credits, 4_000_000 ISA)
        dm.complete_deposit(&d2.id, Hash::hash_data(b"tx2"), 13)
            .unwrap();
        assert_eq!(dm.get_total_deposited(), TWO_ISA + 4_000_000);
        assert_eq!(dm.get_total_credits_issued(), 30_000);
    }

    // ----------------------------------------------------------------
    // test_already_processed
    // ----------------------------------------------------------------

    #[test]
    fn test_already_processed() {
        let mut dm = setup();

        let deposit = dm
            .initiate_deposit(user(), TWO_ISA, ISA_PRICE, 10)
            .unwrap();
        let id = deposit.id;

        dm.complete_deposit(&id, Hash::hash_data(b"tx"), 11)
            .unwrap();

        // Completing again should fail
        let result = dm.complete_deposit(&id, Hash::hash_data(b"tx2"), 12);
        assert_eq!(result, Err(DepositError::AlreadyProcessed));

        // Failing a completed deposit should fail
        let result2 = dm.fail_deposit(&id, "late".to_string());
        assert_eq!(result2, Err(DepositError::AlreadyProcessed));
    }

    // ----------------------------------------------------------------
    // test_deposit_not_found
    // ----------------------------------------------------------------

    #[test]
    fn test_deposit_not_found() {
        let mut dm = setup();
        let fake_id = Hash::hash_data(b"does-not-exist");

        assert_eq!(
            dm.complete_deposit(&fake_id, Hash::hash_data(b"tx"), 1),
            Err(DepositError::DepositNotFound(fake_id))
        );

        assert_eq!(
            dm.fail_deposit(&fake_id, "gone".to_string()),
            Err(DepositError::DepositNotFound(fake_id))
        );

        assert_eq!(
            dm.refund_deposit(&fake_id),
            Err(DepositError::DepositNotFound(fake_id))
        );
    }

    // ----------------------------------------------------------------
    // test_refund_only_failed
    // ----------------------------------------------------------------

    #[test]
    fn test_refund_only_failed() {
        let mut dm = setup();

        // Pending deposit — refund should fail (not in Failed state)
        let deposit = dm
            .initiate_deposit(user(), TWO_ISA, ISA_PRICE, 10)
            .unwrap();
        let id = deposit.id;

        let result = dm.refund_deposit(&id);
        assert_eq!(result, Err(DepositError::AlreadyProcessed));
    }
}
