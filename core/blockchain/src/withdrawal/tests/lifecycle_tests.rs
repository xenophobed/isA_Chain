use super::*;
use crate::types::Hash;
use crate::withdrawal::types::{WithdrawalError, WithdrawalStatus};

#[test]
fn test_initiate_withdrawal() {
    let mut wm = setup();

    let req = wm.initiate_withdrawal(user(), 10_000, ISA_PRICE, 50).unwrap();

    assert_eq!(req.user, user());
    assert_eq!(req.credits_amount, 10_000);
    assert_eq!(req.status, WithdrawalStatus::InCooldown);
    assert_eq!(req.created_at, 50);
    assert_eq!(req.cooldown_ends, 150); // 50 + 100
    assert!(req.tx_hash.is_none());
    assert!(req.completed_at.is_none());

    // Fee = 10_000 * 50 / 10_000 = 50 credits
    assert_eq!(req.fee, 50);

    // net_credits = 9_950; ISA = 9_950 * 10_000 / 500_000 = 199 (integer division)
    let expected_isa = WithdrawalManager::calculate_isa(9_950, ISA_PRICE);
    assert_eq!(req.isa_to_receive, expected_isa);

    // Request should be stored
    let stored = wm.get_withdrawal(&req.id).unwrap();
    assert_eq!(stored.id, req.id);
}

#[test]
fn test_complete_withdrawal() {
    let mut wm = setup();
    let req = wm.initiate_withdrawal(user(), 10_000, ISA_PRICE, 50).unwrap();
    let id = req.id;
    let tx = Hash::hash_data(b"mint-tx");

    // Should fail before cooldown ends (height 149 < 150)
    let err = wm.complete_withdrawal(&id, tx, 149).unwrap_err();
    assert_eq!(err, WithdrawalError::CooldownNotComplete);

    // Should succeed at cooldown_ends (height 150)
    let completed = wm.complete_withdrawal(&id, tx, 150).unwrap();
    assert_eq!(completed.status, WithdrawalStatus::Completed);
    assert_eq!(completed.completed_at, Some(150));
    assert_eq!(completed.tx_hash, Some(tx));

    // Totals updated
    assert_eq!(wm.total_withdrawn_credits, 10_000);
    assert_eq!(wm.total_isa_minted, req.isa_to_receive);
}

#[test]
fn test_cancel_withdrawal() {
    let mut wm = setup();
    let req = wm.initiate_withdrawal(user(), 5_000, ISA_PRICE, 10).unwrap();
    let id = req.id;

    let refund = wm.cancel_withdrawal(&id).unwrap();
    assert_eq!(refund, 5_000); // full credits_amount returned

    let stored = wm.get_withdrawal(&id).unwrap();
    assert_eq!(stored.status, WithdrawalStatus::Cancelled);

    // Cannot cancel again
    let err = wm.cancel_withdrawal(&id).unwrap_err();
    assert_eq!(err, WithdrawalError::AlreadyProcessed);
}

#[test]
fn test_cooldown_check() {
    let mut wm = setup();
    let req = wm.initiate_withdrawal(user(), 2_000, ISA_PRICE, 0).unwrap();
    let id = req.id;

    // cooldown_ends = 0 + 100 = 100
    assert!(!wm.check_cooldown(&id, 99).unwrap());
    assert!(wm.check_cooldown(&id, 100).unwrap());
    assert!(wm.check_cooldown(&id, 200).unwrap());
}

#[test]
fn test_already_processed() {
    let mut wm = setup();
    let req = wm.initiate_withdrawal(user(), 10_000, ISA_PRICE, 0).unwrap();
    let id = req.id;
    let tx = Hash::hash_data(b"tx1");

    // Complete it
    wm.complete_withdrawal(&id, tx, 100).unwrap();

    // Completing again must fail
    let err = wm.complete_withdrawal(&id, tx, 101).unwrap_err();
    assert_eq!(err, WithdrawalError::AlreadyProcessed);

    // Cancelling a completed withdrawal must also fail
    let cancel_err = wm.cancel_withdrawal(&id).unwrap_err();
    assert_eq!(cancel_err, WithdrawalError::AlreadyProcessed);
}
