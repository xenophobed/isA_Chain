use super::*;
use crate::deposit::types::DepositError;
use crate::types::Hash;

#[test]
fn test_zero_amount_fails() {
    let mut dm = setup();

    let result = dm.initiate_deposit(user(), 0, ISA_PRICE, 1);
    assert_eq!(result, Err(DepositError::InvalidAmount));
}

#[test]
fn test_zero_price_fails() {
    let mut dm = setup();

    let result = dm.initiate_deposit(user(), TWO_ISA, 0, 1);
    assert_eq!(result, Err(DepositError::PriceStale));
}

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
