use super::types::*;
use crate::types::{Address, Amount, BlockHeight};
use std::collections::HashMap;

/// The balance sync engine: maintains history, tracks discrepancies, and
/// decides how to reconcile on-chain ISA balances with off-chain credits.
pub struct BalanceSyncEngine {
    /// Ordered history of completed sync passes
    pub sync_history: Vec<SyncResult>,
    /// Most-recent `BalanceRecord` for every address that showed a discrepancy
    pub known_discrepancies: HashMap<Address, BalanceRecord>,
    /// Maximum absolute discrepancy (in micro-credits) that may be fixed
    /// automatically — anything larger is flagged for review
    pub auto_fix_threshold: Amount,
    /// Minimum number of blocks between sync passes
    pub sync_interval_blocks: u64,
    /// Block height of the most-recent completed sync (0 = never synced)
    pub last_sync_height: BlockHeight,
    /// Lifetime count of sync passes
    pub total_syncs: u64,
    /// Lifetime count of automatic credit adjustments applied
    pub total_auto_fixes: u64,
}

impl BalanceSyncEngine {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Create a new engine with the given auto-fix threshold and sync interval.
    pub fn new(auto_fix_threshold: Amount, sync_interval: u64) -> Self {
        Self {
            sync_history: Vec::new(),
            known_discrepancies: HashMap::new(),
            auto_fix_threshold,
            sync_interval_blocks: sync_interval,
            last_sync_height: 0,
            total_syncs: 0,
            total_auto_fixes: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Per-account check
    // -----------------------------------------------------------------------

    /// Build a `BalanceRecord` by comparing `on_chain_isa` (ISA wei) with
    /// `off_chain_credits` (micro-credits).
    ///
    /// The expected credit balance is derived as:
    ///
    /// ```text
    /// isa_usd_value  = on_chain_isa  * isa_price_usd  / 1e18   (ISA wei → USD)
    /// expected       = isa_usd_value * 1e6 / credit_price_usd  (USD → micro-credits)
    /// ```
    ///
    /// Both `isa_price_usd` and `credit_price_usd` are expressed in micro-USD
    /// (i.e. `1_000_000` = $1.00), matching the scale used by `CreditEngine`.
    pub fn check_balance(
        &self,
        address: Address,
        on_chain_isa: Amount,
        off_chain_credits: Amount,
        isa_price_usd: Amount,
        credit_price_usd: Amount,
        height: BlockHeight,
    ) -> BalanceRecord {
        // Avoid division-by-zero: treat zero credit price as "no conversion"
        let expected_credits = if credit_price_usd == 0 || isa_price_usd == 0 {
            0u128
        } else {
            // ISA is stored in wei (18 decimals).  Divide by 1e18 to get whole
            // ISA, then multiply by price in micro-USD, then divide by
            // credit_price_usd to get micro-credits.
            //
            // To keep precision, scale numerator before dividing:
            //   expected = on_chain_isa * isa_price_usd / 1e18 / credit_price_usd
            //            = on_chain_isa * isa_price_usd / (1e18 * credit_price_usd)
            let numerator = on_chain_isa.saturating_mul(isa_price_usd);
            let denominator = 1_000_000_000_000_000_000u128 // 1e18 (ISA decimals)
                .saturating_mul(credit_price_usd);
            if denominator == 0 {
                0
            } else {
                numerator / denominator
            }
        };

        let discrepancy = off_chain_credits as i128 - expected_credits as i128;

        BalanceRecord {
            address,
            on_chain_isa,
            off_chain_credits,
            expected_credits,
            discrepancy,
            last_synced: height,
        }
    }

    // -----------------------------------------------------------------------
    // Action determination
    // -----------------------------------------------------------------------

    /// Decide what to do with a `BalanceRecord`.
    ///
    /// Rules:
    /// - No discrepancy → `NoAction`
    /// - |discrepancy| ≤ `auto_fix_threshold` → `CreditAdjustment`
    /// - |discrepancy| > `auto_fix_threshold` → `FlagForReview`
    pub fn determine_action(&self, record: &BalanceRecord) -> SyncAction {
        if record.discrepancy == 0 {
            return SyncAction::NoAction;
        }

        let abs_discrepancy = record.discrepancy.unsigned_abs();
        if abs_discrepancy <= self.auto_fix_threshold {
            // Negate because the adjustment is "what we need to ADD to reach
            // expected": if we over-credited (positive discrepancy) we deduct.
            SyncAction::CreditAdjustment(-record.discrepancy)
        } else {
            SyncAction::FlagForReview
        }
    }

    // -----------------------------------------------------------------------
    // Batch sync
    // -----------------------------------------------------------------------

    /// Run a full reconciliation pass over the provided balances.
    ///
    /// `balances` is a list of `(address, on_chain_isa, off_chain_credits)`.
    /// Prices are in micro-USD.
    pub fn run_sync(
        &mut self,
        balances: Vec<(Address, Amount, Amount)>,
        isa_price_usd: Amount,
        credit_price_usd: Amount,
        height: BlockHeight,
    ) -> SyncResult {
        let total_accounts = balances.len();
        let mut records = Vec::with_capacity(total_accounts);
        let mut discrepancies_found = 0usize;
        let mut total_discrepancy = 0i128;

        for (address, on_chain_isa, off_chain_credits) in balances {
            let record = self.check_balance(
                address,
                on_chain_isa,
                off_chain_credits,
                isa_price_usd,
                credit_price_usd,
                height,
            );

            if record.discrepancy != 0 {
                discrepancies_found += 1;
                total_discrepancy = total_discrepancy.saturating_add(record.discrepancy);

                let action = self.determine_action(&record);
                if let SyncAction::CreditAdjustment(_) = action {
                    self.total_auto_fixes += 1;
                }

                self.known_discrepancies.insert(address, record.clone());
            }

            records.push(record);
        }

        let result = SyncResult {
            total_accounts,
            synced: total_accounts,
            discrepancies_found,
            total_discrepancy,
            records,
            height,
        };

        self.last_sync_height = height;
        self.total_syncs += 1;
        self.sync_history.push(result.clone());

        result
    }

    // -----------------------------------------------------------------------
    // Scheduling helper
    // -----------------------------------------------------------------------

    /// Returns `true` when enough blocks have elapsed since the last sync.
    pub fn should_sync(&self, current_height: BlockHeight) -> bool {
        if self.last_sync_height == 0 {
            return true;
        }
        current_height.saturating_sub(self.last_sync_height) >= self.sync_interval_blocks
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Return references to all currently-tracked discrepancy records.
    pub fn get_discrepancies(&self) -> Vec<&BalanceRecord> {
        self.known_discrepancies.values().collect()
    }

    /// Mark a discrepancy as resolved (remove from tracking map).
    ///
    /// Returns `true` if the address was present and removed, `false` otherwise.
    pub fn resolve_discrepancy(&mut self, address: &Address) -> bool {
        self.known_discrepancies.remove(address).is_some()
    }

    /// Return the most-recent `SyncResult`, if any.
    pub fn get_last_sync(&self) -> Option<&SyncResult> {
        self.sync_history.last()
    }

    /// Return `(total_syncs, total_auto_fixes)`.
    pub fn get_sync_stats(&self) -> (u64, u64) {
        (self.total_syncs, self.total_auto_fixes)
    }
}
