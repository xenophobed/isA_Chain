use super::helpers::*;
use crate::balance_sync::engine::BalanceSyncEngine;
use crate::balance_sync::types::{SyncAction, DEFAULT_SYNC_INTERVAL_BLOCKS};

// ---- balance checks -----------------------------------------------------

#[test]
fn test_check_balance_match() {
    let eng = engine();
    // 1 ISA at $1.00 with credit = $0.01 → expected 100 credits
    let record = eng.check_balance(addr(1), ONE_ISA, 100, ISA_PRICE, CREDIT_PRICE, 50);
    assert_eq!(record.expected_credits, 100);
    assert_eq!(record.discrepancy, 0);
}

#[test]
fn test_check_balance_discrepancy() {
    let eng = engine();
    // Off-chain has 200 credits but expected is 100 → discrepancy = +100
    let record = eng.check_balance(addr(2), ONE_ISA, 200, ISA_PRICE, CREDIT_PRICE, 50);
    assert_eq!(record.expected_credits, 100);
    assert_eq!(record.discrepancy, 100);
}

#[test]
fn test_check_balance_under_credited() {
    let eng = engine();
    // Off-chain has 50 credits but expected is 100 → discrepancy = -50
    let record = eng.check_balance(addr(3), ONE_ISA, 50, ISA_PRICE, CREDIT_PRICE, 50);
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
        (addr(1), ONE_ISA, 100u128), // matches
        (addr(2), ONE_ISA, 110u128), // over by 10
        (addr(3), ONE_ISA, 90u128),  // under by 10
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
