use super::*;
use crate::settlement_fallback::types::FallbackAction;

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
