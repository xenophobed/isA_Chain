use super::*;
use crate::deposit::types::{DepositError, DepositStatus};
use crate::types::Hash;

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
