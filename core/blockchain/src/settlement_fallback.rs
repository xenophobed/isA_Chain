use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Constants
// ============================================================================

/// Default number of blocks before a deferred settlement expires.
pub const DEFAULT_MAX_DEFER_BLOCKS: u64 = 1_000;

/// Default shortfall threshold below which credits are auto-deducted.
pub const DEFAULT_AUTO_CREDIT_THRESHOLD: Amount = 1_000_000;

// ============================================================================
// FallbackAction
// ============================================================================

/// The action taken when a user has insufficient on-chain ISA balance at
/// settlement time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FallbackAction {
    /// Deduct the shortfall from the user's off-chain credit balance instead.
    CreditDeduction,

    /// Settle the available portion now; record the remainder as outstanding.
    PartialSettlement {
        /// Amount that was successfully settled.
        settled: Amount,
        /// Amount still owed (the shortfall).
        remaining: Amount,
    },

    /// Defer the full settlement until `deadline` block height.
    DeferredSettlement {
        /// Block height by which the settlement must be resolved.
        deadline: BlockHeight,
    },

    /// The provider extends credit to the user (trusted relationship).
    ProviderCredit,

    /// Settlement rejected entirely — no fallback available.
    Rejected,
}

// ============================================================================
// FallbackRecord
// ============================================================================

/// Immutable record of a single fallback event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FallbackRecord {
    /// Unique ID for this fallback record.
    pub id: Hash,
    /// ID of the original settlement that triggered the fallback.
    pub original_settlement_id: Hash,
    /// The user who could not cover the settlement.
    pub user: Address,
    /// The provider who was owed the settlement amount.
    pub provider: Address,
    /// The full amount the settlement required.
    pub required_amount: Amount,
    /// The user's on-chain ISA balance at the time of the shortfall.
    pub available_balance: Amount,
    /// `required_amount - available_balance`.
    pub shortfall: Amount,
    /// Which fallback strategy was applied.
    pub action_taken: FallbackAction,
    /// Block height at which the fallback was recorded.
    pub height: BlockHeight,
    /// Whether this record has been fully resolved.
    pub resolved: bool,
    /// Block height at which this record was resolved, if applicable.
    pub resolved_at: Option<BlockHeight>,
}

// ============================================================================
// FallbackError
// ============================================================================

/// Errors that can arise during fallback handling.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FallbackError {
    #[error("No fallback available for the given shortfall")]
    NoFallbackAvailable,

    #[error("Fallback record already resolved: {0:?}")]
    AlreadyResolved(Hash),

    #[error("Fallback record not found: {0:?}")]
    RecordNotFound(Hash),

    #[error("Deferred settlement deadline has expired")]
    DeadlineExpired,

    #[error("Insufficient credits to cover the shortfall")]
    InsufficientCredits,
}

// ============================================================================
// FallbackManager
// ============================================================================

/// Manages on-chain fallback logic for settlements that cannot be covered by
/// the user's ISA balance.
///
/// # Decision logic (`handle_shortfall`)
///
/// 1. **shortfall ≤ `auto_credit_threshold` AND `credit_balance ≥ shortfall`**
///    → `CreditDeduction` (instant, no on-chain ISA required)
/// 2. **`available_balance > 0`**
///    → `PartialSettlement` (settle what's available, record the remainder)
/// 3. **Otherwise**
///    → `DeferredSettlement` with `deadline = height + max_defer_blocks`
pub struct FallbackManager {
    /// All fallback records, keyed by their unique ID.
    pub records: HashMap<Hash, FallbackRecord>,

    /// IDs of records in `DeferredSettlement` state that are not yet resolved.
    pub deferred: Vec<Hash>,

    /// Index: user address → list of fallback record IDs.
    pub by_user: HashMap<Address, Vec<Hash>>,

    /// Running total of all shortfall amounts recorded.
    pub total_shortfall: Amount,

    /// Running total of amounts that have been recovered (resolved records).
    pub total_recovered: Amount,

    /// Maximum number of blocks a deferred settlement may remain open.
    pub max_defer_blocks: u64,

    /// If the shortfall is at or below this amount **and** the user has
    /// sufficient credits, automatically deduct from credits.
    pub auto_credit_threshold: Amount,
}

impl FallbackManager {
    // ----------------------------------------------------------------
    // Constructor
    // ----------------------------------------------------------------

    /// Create a new `FallbackManager`.
    ///
    /// - `max_defer_blocks`: how many blocks before a deferred settlement expires
    ///   (use [`DEFAULT_MAX_DEFER_BLOCKS`] for 1 000 blocks).
    /// - `auto_credit_threshold`: shortfall size at or below which credits are
    ///   auto-deducted (use [`DEFAULT_AUTO_CREDIT_THRESHOLD`] for 1 000 000).
    pub fn new(max_defer_blocks: u64, auto_credit_threshold: Amount) -> Self {
        FallbackManager {
            records: HashMap::new(),
            deferred: Vec::new(),
            by_user: HashMap::new(),
            total_shortfall: 0,
            total_recovered: 0,
            max_defer_blocks,
            auto_credit_threshold,
        }
    }

    // ----------------------------------------------------------------
    // Core fallback decision
    // ----------------------------------------------------------------

    /// Evaluate a settlement shortfall and choose the best fallback strategy.
    ///
    /// Returns the created [`FallbackRecord`] on success.
    ///
    /// # Arguments
    /// - `settlement_id` — ID of the settlement that triggered the shortfall.
    /// - `user` — payer address.
    /// - `provider` — recipient address.
    /// - `required` — full amount required by the settlement.
    /// - `available_balance` — user's current on-chain ISA balance.
    /// - `credit_balance` — user's current off-chain credit balance.
    /// - `height` — current block height.
    pub fn handle_shortfall(
        &mut self,
        settlement_id: Hash,
        user: Address,
        provider: Address,
        required: Amount,
        available_balance: Amount,
        credit_balance: Amount,
        height: BlockHeight,
    ) -> Result<FallbackRecord, FallbackError> {
        let shortfall = required.saturating_sub(available_balance);

        // Decide which action to take.
        let action = if shortfall <= self.auto_credit_threshold && credit_balance >= shortfall {
            FallbackAction::CreditDeduction
        } else if available_balance > 0 {
            FallbackAction::PartialSettlement {
                settled: available_balance,
                remaining: shortfall,
            }
        } else {
            FallbackAction::DeferredSettlement {
                deadline: height + self.max_defer_blocks,
            }
        };

        // Generate a deterministic record ID.
        let mut id_input = Vec::new();
        id_input.extend_from_slice(settlement_id.as_bytes());
        id_input.extend_from_slice(user.as_bytes());
        id_input.extend_from_slice(provider.as_bytes());
        id_input.extend_from_slice(&required.to_le_bytes());
        id_input.extend_from_slice(&height.to_le_bytes());
        id_input.extend_from_slice(&(self.records.len() as u64).to_le_bytes());
        let id = Hash::hash_data(&id_input);

        let record = FallbackRecord {
            id,
            original_settlement_id: settlement_id,
            user,
            provider,
            required_amount: required,
            available_balance,
            shortfall,
            action_taken: action.clone(),
            height,
            resolved: false,
            resolved_at: None,
        };

        // Update indices and running totals.
        self.total_shortfall = self.total_shortfall.saturating_add(shortfall);

        if matches!(action, FallbackAction::DeferredSettlement { .. }) {
            self.deferred.push(id);
        }

        self.by_user.entry(user).or_default().push(id);
        self.records.insert(id, record.clone());

        Ok(record)
    }

    // ----------------------------------------------------------------
    // Resolution
    // ----------------------------------------------------------------

    /// Mark a deferred fallback record as resolved.
    ///
    /// Fails if:
    /// - The record does not exist ([`FallbackError::RecordNotFound`]).
    /// - The record was already resolved ([`FallbackError::AlreadyResolved`]).
    /// - The current block height is past the deadline ([`FallbackError::DeadlineExpired`]).
    pub fn resolve_deferred(
        &mut self,
        record_id: &Hash,
        current_height: BlockHeight,
    ) -> Result<(), FallbackError> {
        let record = self
            .records
            .get_mut(record_id)
            .ok_or(FallbackError::RecordNotFound(*record_id))?;

        if record.resolved {
            return Err(FallbackError::AlreadyResolved(*record_id));
        }

        // Check deadline only for deferred records.
        if let FallbackAction::DeferredSettlement { deadline } = &record.action_taken {
            if current_height > *deadline {
                return Err(FallbackError::DeadlineExpired);
            }
        }

        record.resolved = true;
        record.resolved_at = Some(current_height);

        // Update recovered total.
        self.total_recovered = self
            .total_recovered
            .saturating_add(record.shortfall);

        // Remove from the deferred list.
        self.deferred.retain(|id| id != record_id);

        Ok(())
    }

    // ----------------------------------------------------------------
    // Expiry
    // ----------------------------------------------------------------

    /// Expire all deferred records whose deadline has passed.
    ///
    /// Returns the IDs of the records that were expired (they remain in
    /// `records` as unresolved but are removed from `deferred`).
    pub fn expire_deferred(&mut self, current_height: BlockHeight) -> Vec<Hash> {
        let mut expired = Vec::new();

        self.deferred.retain(|id| {
            if let Some(record) = self.records.get(id) {
                if let FallbackAction::DeferredSettlement { deadline } = &record.action_taken {
                    if current_height > *deadline {
                        expired.push(*id);
                        return false; // remove from deferred
                    }
                }
            }
            true // keep
        });

        expired
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    /// Look up a fallback record by its ID.
    pub fn get_record(&self, id: &Hash) -> Option<&FallbackRecord> {
        self.records.get(id)
    }

    /// All fallback records associated with `user`.
    pub fn get_user_records(&self, user: &Address) -> Vec<&FallbackRecord> {
        self.by_user
            .get(user)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.records.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// All deferred records that have not yet been resolved.
    pub fn get_pending_deferred(&self) -> Vec<&FallbackRecord> {
        self.deferred
            .iter()
            .filter_map(|id| self.records.get(id))
            .collect()
    }

    /// Running total of all shortfall amounts.
    pub fn get_total_shortfall(&self) -> Amount {
        self.total_shortfall
    }

    /// Running total of all recovered (resolved) shortfall amounts.
    pub fn get_total_recovered(&self) -> Amount {
        self.total_recovered
    }

    /// Recovery rate expressed in basis points (0–10 000).
    ///
    /// Returns 0 when `total_shortfall` is zero (no shortfalls recorded yet).
    pub fn get_recovery_rate_bps(&self) -> u32 {
        if self.total_shortfall == 0 {
            return 0;
        }
        // recovered / shortfall * 10_000, clamped to u32::MAX
        let bps = self
            .total_recovered
            .saturating_mul(10_000)
            / self.total_shortfall;
        bps.min(u128::from(u32::MAX)) as u32
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Fixtures ----------------------------------------------------------

    fn settlement_id() -> Hash {
        Hash::hash_data(b"settlement_1")
    }

    fn user() -> Address {
        Address::from([0x11; 20])
    }

    fn provider() -> Address {
        Address::from([0x22; 20])
    }

    fn manager() -> FallbackManager {
        FallbackManager::new(DEFAULT_MAX_DEFER_BLOCKS, DEFAULT_AUTO_CREDIT_THRESHOLD)
    }

    const REQUIRED: Amount = 5_000_000;

    // ---- test_credit_deduction_fallback ------------------------------------

    #[test]
    fn test_credit_deduction_fallback() {
        let mut m = manager();

        // shortfall = 5_000_000 - 4_500_000 = 500_000 (≤ threshold 1_000_000)
        // credit_balance = 600_000 ≥ shortfall → CreditDeduction
        let rec = m
            .handle_shortfall(
                settlement_id(),
                user(),
                provider(),
                REQUIRED,
                4_500_000, // available_balance
                600_000,   // credit_balance
                10,
            )
            .unwrap();

        assert_eq!(rec.action_taken, FallbackAction::CreditDeduction);
        assert_eq!(rec.shortfall, 500_000);
        assert!(!rec.resolved);
        assert_eq!(m.get_total_shortfall(), 500_000);
        // CreditDeduction does not go into the deferred list.
        assert!(m.get_pending_deferred().is_empty());
    }

    // ---- test_partial_settlement -------------------------------------------

    #[test]
    fn test_partial_settlement() {
        let mut m = manager();

        // shortfall = 5_000_000 - 2_000_000 = 3_000_000 (> threshold 1_000_000)
        // available_balance = 2_000_000 > 0 → PartialSettlement
        let rec = m
            .handle_shortfall(
                settlement_id(),
                user(),
                provider(),
                REQUIRED,
                2_000_000, // available_balance
                0,         // no credits
                20,
            )
            .unwrap();

        assert_eq!(
            rec.action_taken,
            FallbackAction::PartialSettlement {
                settled: 2_000_000,
                remaining: 3_000_000,
            }
        );
        assert_eq!(rec.shortfall, 3_000_000);
        assert_eq!(m.get_total_shortfall(), 3_000_000);
    }

    // ---- test_deferred_settlement ------------------------------------------

    #[test]
    fn test_deferred_settlement() {
        let mut m = manager();

        // available_balance = 0, shortfall > threshold → DeferredSettlement
        let rec = m
            .handle_shortfall(
                settlement_id(),
                user(),
                provider(),
                REQUIRED,
                0, // no on-chain balance
                0, // no credits
                50,
            )
            .unwrap();

        assert_eq!(
            rec.action_taken,
            FallbackAction::DeferredSettlement {
                deadline: 50 + DEFAULT_MAX_DEFER_BLOCKS,
            }
        );
        assert_eq!(rec.shortfall, REQUIRED);
        assert_eq!(m.get_pending_deferred().len(), 1);
    }

    // ---- test_resolve_deferred --------------------------------------------

    #[test]
    fn test_resolve_deferred() {
        let mut m = manager();

        let rec = m
            .handle_shortfall(settlement_id(), user(), provider(), REQUIRED, 0, 0, 100)
            .unwrap();

        // Resolve before deadline (100 + 1_000 = 1_100)
        m.resolve_deferred(&rec.id, 500).unwrap();

        let updated = m.get_record(&rec.id).unwrap();
        assert!(updated.resolved);
        assert_eq!(updated.resolved_at, Some(500));

        // Should no longer be in the deferred list.
        assert!(m.get_pending_deferred().is_empty());

        // Recovery tracking.
        assert_eq!(m.get_total_recovered(), REQUIRED);
    }

    // ---- test_expire_deferred ---------------------------------------------

    #[test]
    fn test_expire_deferred() {
        let mut m = manager();

        let rec = m
            .handle_shortfall(settlement_id(), user(), provider(), REQUIRED, 0, 0, 10)
            .unwrap();

        // Deadline = 10 + 1_000 = 1_010.  Expire at height 1_011.
        let expired = m.expire_deferred(1_011);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0], rec.id);

        // The record is still in `records` but removed from `deferred`.
        assert!(m.get_pending_deferred().is_empty());
        assert!(m.get_record(&rec.id).is_some());
    }

    // ---- test_already_resolved --------------------------------------------

    #[test]
    fn test_already_resolved() {
        let mut m = manager();

        let rec = m
            .handle_shortfall(settlement_id(), user(), provider(), REQUIRED, 0, 0, 5)
            .unwrap();

        m.resolve_deferred(&rec.id, 100).unwrap();

        let result = m.resolve_deferred(&rec.id, 200);
        assert_eq!(result, Err(FallbackError::AlreadyResolved(rec.id)));
    }

    // ---- test_no_balance_no_credits ----------------------------------------

    #[test]
    fn test_no_balance_no_credits() {
        let mut m = manager();

        // Both balances are zero → DeferredSettlement (not Rejected in this impl).
        let rec = m
            .handle_shortfall(settlement_id(), user(), provider(), REQUIRED, 0, 0, 1)
            .unwrap();

        assert!(matches!(
            rec.action_taken,
            FallbackAction::DeferredSettlement { .. }
        ));
    }

    // ---- test_recovery_rate ------------------------------------------------

    #[test]
    fn test_recovery_rate() {
        let mut m = manager();

        // Record two shortfalls of 1_000_000 each → total 2_000_000.
        let rec1 = m
            .handle_shortfall(Hash::hash_data(b"s1"), user(), provider(), REQUIRED, 0, 0, 1)
            .unwrap();

        let rec2 = m
            .handle_shortfall(Hash::hash_data(b"s2"), user(), provider(), REQUIRED, 0, 0, 2)
            .unwrap();

        // Resolve only the first.
        m.resolve_deferred(&rec1.id, 10).unwrap();

        // recovered = REQUIRED (5_000_000), shortfall = 2 * REQUIRED (10_000_000)
        // rate = 5_000_000 / 10_000_000 * 10_000 = 5_000 bps
        assert_eq!(m.get_recovery_rate_bps(), 5_000);

        // Resolve the second.
        m.resolve_deferred(&rec2.id, 20).unwrap();
        assert_eq!(m.get_recovery_rate_bps(), 10_000);
    }

    // ---- test_total_tracking -----------------------------------------------

    #[test]
    fn test_total_tracking() {
        let mut m = manager();

        m.handle_shortfall(
            Hash::hash_data(b"s1"),
            user(),
            provider(),
            3_000_000,
            1_000_000, // shortfall = 2_000_000
            0,
            1,
        )
        .unwrap();

        m.handle_shortfall(
            Hash::hash_data(b"s2"),
            user(),
            provider(),
            2_000_000,
            0, // shortfall = 2_000_000
            0,
            2,
        )
        .unwrap();

        assert_eq!(m.get_total_shortfall(), 4_000_000);
        assert_eq!(m.get_total_recovered(), 0);
        assert_eq!(m.get_recovery_rate_bps(), 0);
    }

    // ---- test_get_user_records ---------------------------------------------

    #[test]
    fn test_get_user_records() {
        let mut m = manager();

        let user2 = Address::from([0x33; 20]);

        // Two records for `user()`.
        m.handle_shortfall(Hash::hash_data(b"s1"), user(), provider(), REQUIRED, 0, 0, 1)
            .unwrap();
        m.handle_shortfall(Hash::hash_data(b"s2"), user(), provider(), REQUIRED, 0, 0, 2)
            .unwrap();

        // One record for `user2`.
        m.handle_shortfall(Hash::hash_data(b"s3"), user2, provider(), REQUIRED, 0, 0, 3)
            .unwrap();

        let recs = m.get_user_records(&user());
        assert_eq!(recs.len(), 2);
        for r in &recs {
            assert_eq!(r.user, user());
        }

        let recs2 = m.get_user_records(&user2);
        assert_eq!(recs2.len(), 1);
    }

    // ---- test_shortfall_below_threshold ------------------------------------

    #[test]
    fn test_shortfall_below_threshold() {
        let mut m = manager();

        // shortfall = 100 (well below 1_000_000 threshold)
        // credit_balance = 200 ≥ 100 → CreditDeduction
        let rec = m
            .handle_shortfall(
                settlement_id(),
                user(),
                provider(),
                1_000,
                900,   // available_balance → shortfall = 100
                200,   // credit_balance ≥ shortfall
                1,
            )
            .unwrap();

        assert_eq!(rec.action_taken, FallbackAction::CreditDeduction);
        assert_eq!(rec.shortfall, 100);
    }

    // ---- test_record_not_found --------------------------------------------

    #[test]
    fn test_record_not_found() {
        let mut m = manager();
        let unknown = Hash::hash_data(b"does_not_exist");
        let result = m.resolve_deferred(&unknown, 100);
        assert_eq!(result, Err(FallbackError::RecordNotFound(unknown)));
    }

    // ---- test_deadline_expired --------------------------------------------

    #[test]
    fn test_deadline_expired() {
        let mut m = manager();

        let rec = m
            .handle_shortfall(settlement_id(), user(), provider(), REQUIRED, 0, 0, 10)
            .unwrap();

        // deadline = 10 + 1_000 = 1_010; try to resolve at 1_011 (past deadline)
        let result = m.resolve_deferred(&rec.id, 1_011);
        assert_eq!(result, Err(FallbackError::DeadlineExpired));
    }
}
