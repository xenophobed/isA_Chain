use super::*;
use crate::types::{Address, Hash};
use crate::withdrawal::types::{WithdrawalError, DEFAULT_COOLDOWN_BLOCKS, DEFAULT_WITHDRAWAL_FEE_BPS};

#[test]
fn test_below_minimum_fails() {
    let mut wm = setup();

    let err = wm
        .initiate_withdrawal(user(), 999, ISA_PRICE, 1)
        .unwrap_err();

    assert_eq!(
        err,
        WithdrawalError::MinimumNotMet {
            minimum: DEFAULT_MIN_WITHDRAWAL,
            requested: 999
        }
    );
}

#[test]
fn test_fee_calculation() {
    let wm = setup();

    // 0.5% of 10_000 = 50
    assert_eq!(wm.calculate_fee(10_000), 50);

    // 0.5% of 1_000 = 5
    assert_eq!(wm.calculate_fee(1_000), 5);

    // 0.5% of 1_000_000 = 5_000
    assert_eq!(wm.calculate_fee(1_000_000), 5_000);

    // Zero credits → zero fee
    assert_eq!(wm.calculate_fee(0), 0);
}

#[test]
fn test_isa_calculation() {
    // 10_000 credits × $0.00001/credit / $0.50/ISA
    // = 10_000 * 100 / 500_000 = 2 ISA units
    assert_eq!(WithdrawalManager::calculate_isa(10_000, ISA_PRICE), 2);

    // At $1.00/ISA: 10_000 * 100 / 1_000_000 = 1
    assert_eq!(WithdrawalManager::calculate_isa(10_000, 1_000_000), 1);

    // Zero ISA price → 0 (safe)
    assert_eq!(WithdrawalManager::calculate_isa(10_000, 0), 0);

    // Zero credits → 0
    assert_eq!(WithdrawalManager::calculate_isa(0, ISA_PRICE), 0);
}

#[test]
fn test_user_withdrawals() {
    let mut wm = setup();

    // Three requests from user(), one from user2()
    wm.initiate_withdrawal(user(), 1_000, ISA_PRICE, 1).unwrap();
    wm.initiate_withdrawal(user(), 2_000, ISA_PRICE, 2).unwrap();
    wm.initiate_withdrawal(user(), 3_000, ISA_PRICE, 3).unwrap();
    wm.initiate_withdrawal(user2(), 5_000, ISA_PRICE, 4).unwrap();

    let user_reqs = wm.get_user_withdrawals(&user());
    assert_eq!(user_reqs.len(), 3);

    let user2_reqs = wm.get_user_withdrawals(&user2());
    assert_eq!(user2_reqs.len(), 1);

    // No requests for unknown address
    let no_reqs = wm.get_user_withdrawals(&Address::from([0xFF; 20]));
    assert!(no_reqs.is_empty());
}

#[test]
fn test_total_tracking() {
    let mut wm = setup();
    let tx = Hash::hash_data(b"tx");

    let r1 = wm.initiate_withdrawal(user(), 10_000, ISA_PRICE, 0).unwrap();
    let r2 = wm.initiate_withdrawal(user2(), 20_000, ISA_PRICE, 1).unwrap();

    wm.complete_withdrawal(&r1.id, tx, 100).unwrap();
    wm.complete_withdrawal(&r2.id, tx, 101).unwrap();

    assert_eq!(wm.total_withdrawn_credits, 30_000);
    assert_eq!(
        wm.total_isa_minted,
        r1.isa_to_receive + r2.isa_to_receive
    );
}

#[test]
fn test_invalid_amount_fails() {
    let mut wm = setup();
    let err = wm
        .initiate_withdrawal(user(), 0, ISA_PRICE, 1)
        .unwrap_err();
    assert_eq!(err, WithdrawalError::InvalidAmount);
}

#[test]
fn test_withdrawal_not_found() {
    let mut wm = setup();
    let phantom = Hash::hash_data(b"nonexistent");

    assert_eq!(
        wm.complete_withdrawal(&phantom, phantom, 999).unwrap_err(),
        WithdrawalError::WithdrawalNotFound(phantom)
    );
    assert_eq!(
        wm.cancel_withdrawal(&phantom).unwrap_err(),
        WithdrawalError::WithdrawalNotFound(phantom)
    );
    assert_eq!(
        wm.check_cooldown(&phantom, 999).unwrap_err(),
        WithdrawalError::WithdrawalNotFound(phantom)
    );
}

#[test]
fn test_daily_limit_exceeded() {
    let mut wm = WithdrawalManager::new(
        DEFAULT_WITHDRAWAL_FEE_BPS,
        DEFAULT_COOLDOWN_BLOCKS,
        1_000,
        50_000, // tight daily limit: 50_000 credits
        crate::credits::DEFAULT_CREDIT_PRICE_USD,
    );

    // First withdrawal: 30_000 — within limit
    wm.initiate_withdrawal(user(), 30_000, ISA_PRICE, 1).unwrap();

    // Second withdrawal: 25_000 — total would be 55_000 > 50_000
    let err = wm
        .initiate_withdrawal(user(), 25_000, ISA_PRICE, 2)
        .unwrap_err();

    assert_eq!(
        err,
        WithdrawalError::DailyLimitExceeded {
            limit: 50_000,
            used: 30_000
        }
    );

    // A different user is unaffected
    wm.initiate_withdrawal(user2(), 40_000, ISA_PRICE, 3).unwrap();
}
