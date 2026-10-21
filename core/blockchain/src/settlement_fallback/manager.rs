use super::types::{FallbackAction, FallbackError, FallbackRecord};
use crate::types::{Address, Amount, BlockHeight, Hash};
use std::collections::HashMap;

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
    #[allow(clippy::too_many_arguments)]
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
