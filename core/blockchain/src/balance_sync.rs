use crate::types::{Address, Amount, BlockHeight};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// ============================================================================
// Constants
// ============================================================================

/// Default auto-fix threshold: 1_000_000 (small discrepancies only)
pub const DEFAULT_AUTO_FIX_THRESHOLD: Amount = 1_000_000;

/// Default sync interval: every 100 blocks
pub const DEFAULT_SYNC_INTERVAL_BLOCKS: u64 = 100;

// ============================================================================
// Errors
// ============================================================================

/// Errors that can occur during balance synchronisation
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SyncError {
    #[error("Account not found: {0}")]
    AccountNotFound(Address),

    #[error("Sync already in progress")]
    SyncInProgress,

    #[error("Invalid state for sync operation")]
    InvalidState,
}

// ============================================================================
// Core data structures
// ============================================================================

/// A snapshot of one account's balance state at a given block height,
/// together with the computed discrepancy between on-chain ISA value and
/// off-chain credits.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalanceRecord {
    /// Account address
    pub address: Address,
    /// On-chain ISA token balance (in ISA wei)
    pub on_chain_isa: Amount,
    /// Current off-chain credit balance (in micro-credits)
    pub off_chain_credits: Amount,
    /// What the off-chain credit balance *should* be based on the ISA value
    /// at the current oracle prices
    pub expected_credits: Amount,
    /// `off_chain_credits as i128 - expected_credits as i128`
    /// Positive → account was over-credited; negative → under-credited
    pub discrepancy: i128,
    /// Block height at which this record was produced
    pub last_synced: BlockHeight,
}

/// Aggregate result of one full reconciliation pass
#[derive(Clone, Debug)]
pub struct SyncResult {
    /// Total number of accounts examined
    pub total_accounts: usize,
    /// Number of accounts successfully processed (no error)
    pub synced: usize,
    /// Number of accounts where a discrepancy was detected
    pub discrepancies_found: usize,
    /// Sum of all `discrepancy` values across the pass (can be negative)
    pub total_discrepancy: i128,
    /// Individual records for every account examined
    pub records: Vec<BalanceRecord>,
    /// Block height at which the sync ran
    pub height: BlockHeight,
}

// ============================================================================
// Action
// ============================================================================

/// The recommended action after examining a single `BalanceRecord`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncAction {
    /// Balances match — nothing to do
    NoAction,
    /// Apply a credit adjustment; positive = add credits, negative = deduct
    CreditAdjustment(i128),
    /// Discrepancy is too large for automatic resolution — needs human review
    FlagForReview,
    /// Account has pending settlements; skip this cycle
    PendingSettlement,
}

// ============================================================================
// Engine
// ============================================================================

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

        let discrepancy =
            off_chain_credits as i128 - expected_credits as i128;

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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- helpers ------------------------------------------------------------

    fn addr(seed: u8) -> Address {
        Address::new([seed; 20])
    }

    /// 1 ISA = 1e18 wei; price = $1.00 (1_000_000 micro-USD).
    /// 1 credit = $0.01 (10_000 micro-USD).
    /// So 1 ISA → 100 credits.
    const ISA_PRICE: Amount = 1_000_000; // $1.00
    const CREDIT_PRICE: Amount = 10_000; // $0.01
    const ONE_ISA: Amount = 1_000_000_000_000_000_000; // 1e18 wei

    fn engine() -> BalanceSyncEngine {
        BalanceSyncEngine::new(DEFAULT_AUTO_FIX_THRESHOLD, DEFAULT_SYNC_INTERVAL_BLOCKS)
    }

    // ---- balance checks -----------------------------------------------------

    #[test]
    fn test_check_balance_match() {
        let eng = engine();
        // 1 ISA at $1.00 with credit = $0.01 → expected 100 credits
        let record =
            eng.check_balance(addr(1), ONE_ISA, 100, ISA_PRICE, CREDIT_PRICE, 50);
        assert_eq!(record.expected_credits, 100);
        assert_eq!(record.discrepancy, 0);
    }

    #[test]
    fn test_check_balance_discrepancy() {
        let eng = engine();
        // Off-chain has 200 credits but expected is 100 → discrepancy = +100
        let record =
            eng.check_balance(addr(2), ONE_ISA, 200, ISA_PRICE, CREDIT_PRICE, 50);
        assert_eq!(record.expected_credits, 100);
        assert_eq!(record.discrepancy, 100);
    }

    #[test]
    fn test_check_balance_under_credited() {
        let eng = engine();
        // Off-chain has 50 credits but expected is 100 → discrepancy = -50
        let record =
            eng.check_balance(addr(3), ONE_ISA, 50, ISA_PRICE, CREDIT_PRICE, 50);
        assert_eq!(record.discrepancy, -50);
    }

    // ---- action determination -----------------------------------------------

    #[test]
    fn test_determine_action_no_action() {
        let eng = engine();
        let record = eng.check_balance(addr(1), ONE_ISA, 100, ISA_PRICE, CREDIT_PRICE, 1);
        assert_eq!(eng.determine_action(&record), SyncAction::NoAction);
    }

    #[test]
    fn test_determine_action_auto_fix() {
        let eng = engine();
        // Discrepancy of +10 — well within the 1_000_000 threshold
        let record = eng.check_balance(addr(2), ONE_ISA, 110, ISA_PRICE, CREDIT_PRICE, 1);
        // discrepancy = +10 → action should deduct 10 credits
        assert_eq!(eng.determine_action(&record), SyncAction::CreditAdjustment(-10));
    }

    #[test]
    fn test_determine_action_flag_review() {
        // Set a very small threshold so any real discrepancy triggers a flag
        let eng = BalanceSyncEngine::new(1, DEFAULT_SYNC_INTERVAL_BLOCKS);
        // Discrepancy = +100, threshold = 1 → flag for review
        let record = eng.check_balance(addr(3), ONE_ISA, 200, ISA_PRICE, CREDIT_PRICE, 1);
        assert_eq!(eng.determine_action(&record), SyncAction::FlagForReview);
    }

    // ---- batch sync ---------------------------------------------------------

    #[test]
    fn test_run_sync() {
        let mut eng = engine();
        let balances = vec![
            (addr(1), ONE_ISA, 100u128),   // matches
            (addr(2), ONE_ISA, 110u128),   // over by 10
            (addr(3), ONE_ISA, 90u128),    // under by 10
        ];
        let result = eng.run_sync(balances, ISA_PRICE, CREDIT_PRICE, 100);

        assert_eq!(result.total_accounts, 3);
        assert_eq!(result.synced, 3);
        assert_eq!(result.discrepancies_found, 2);
        // +10 + (-10) = 0
        assert_eq!(result.total_discrepancy, 0);
        assert_eq!(result.height, 100);
    }

    // ---- scheduling ---------------------------------------------------------

    #[test]
    fn test_should_sync() {
        let mut eng = engine();
        // Never synced → should sync
        assert!(eng.should_sync(0));

        // Run a sync at block 100
        eng.run_sync(vec![], ISA_PRICE, CREDIT_PRICE, 100);

        // Block 199: interval not elapsed yet
        assert!(!eng.should_sync(199));
        // Block 200: exactly at interval
        assert!(eng.should_sync(200));
        // Block 201: past interval
        assert!(eng.should_sync(201));
    }

    // ---- discrepancy management ---------------------------------------------

    #[test]
    fn test_resolve_discrepancy() {
        let mut eng = engine();
        let balances = vec![(addr(1), ONE_ISA, 500u128)]; // big discrepancy
        eng.run_sync(balances, ISA_PRICE, CREDIT_PRICE, 10);

        assert_eq!(eng.get_discrepancies().len(), 1);

        let resolved = eng.resolve_discrepancy(&addr(1));
        assert!(resolved);
        assert_eq!(eng.get_discrepancies().len(), 0);

        // Resolving again returns false
        assert!(!eng.resolve_discrepancy(&addr(1)));
    }

    // ---- stats --------------------------------------------------------------

    #[test]
    fn test_sync_stats() {
        let mut eng = engine();
        let (s0, f0) = eng.get_sync_stats();
        assert_eq!((s0, f0), (0, 0));

        // Sync with one auto-fixable discrepancy
        eng.run_sync(vec![(addr(1), ONE_ISA, 110u128)], ISA_PRICE, CREDIT_PRICE, 100);
        let (s1, f1) = eng.get_sync_stats();
        assert_eq!(s1, 1);
        assert_eq!(f1, 1); // one auto-fix applied

        // Sync with no discrepancies
        eng.run_sync(vec![(addr(2), ONE_ISA, 100u128)], ISA_PRICE, CREDIT_PRICE, 200);
        let (s2, f2) = eng.get_sync_stats();
        assert_eq!(s2, 2);
        assert_eq!(f2, 1); // still one auto-fix total
    }

    #[test]
    fn test_large_discrepancy() {
        let eng = BalanceSyncEngine::new(50, DEFAULT_SYNC_INTERVAL_BLOCKS);
        // Discrepancy of 10_000 credits — above threshold of 50
        let record = eng.check_balance(addr(9), ONE_ISA, 10_100, ISA_PRICE, CREDIT_PRICE, 1);
        // expected = 100; discrepancy = +10_000
        assert_eq!(record.discrepancy, 10_000);
        assert_eq!(eng.determine_action(&record), SyncAction::FlagForReview);
    }

    #[test]
    fn test_get_last_sync_empty() {
        let eng = engine();
        assert!(eng.get_last_sync().is_none());
    }

    #[test]
    fn test_get_last_sync_returns_most_recent() {
        let mut eng = engine();
        eng.run_sync(vec![], ISA_PRICE, CREDIT_PRICE, 10);
        eng.run_sync(vec![], ISA_PRICE, CREDIT_PRICE, 110);

        let last = eng.get_last_sync().unwrap();
        assert_eq!(last.height, 110);
    }
}
