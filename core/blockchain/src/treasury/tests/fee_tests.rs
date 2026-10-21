use super::*;
use crate::treasury::types::TreasuryError;

#[test]
fn test_collect_fee() {
    let mut t = default_treasury();
    // 2.5% of 10_000 = 250
    let fee = t.collect_fee(10_000).unwrap();
    assert_eq!(fee, 250);
    assert_eq!(t.get_balance(), 250);
    assert_eq!(t.get_total_collected(), 250);
}

#[test]
fn test_collect_fee_zero_fails() {
    let mut t = default_treasury();
    let result = t.collect_fee(0);
    assert_eq!(result, Err(TreasuryError::ZeroAmount));
}

/// A gross amount so small that the fee rounds to zero should also fail.
#[test]
fn test_collect_fee_rounds_to_zero_fails() {
    let mut t = default_treasury();
    // 1 * 250 / 10_000 == 0 (integer division)
    let result = t.collect_fee(1);
    assert_eq!(result, Err(TreasuryError::ZeroAmount));
}

#[test]
fn test_deposit() {
    let mut t = default_treasury();
    t.deposit(1_000).unwrap();
    assert_eq!(t.get_balance(), 1_000);
    assert_eq!(t.get_total_collected(), 1_000);
}

#[test]
fn test_deposit_zero_fails() {
    let mut t = default_treasury();
    assert_eq!(t.deposit(0), Err(TreasuryError::ZeroAmount));
}

#[test]
fn test_calculate_fee() {
    let t = default_treasury();
    // 2.5% of 1_000_000 = 25_000
    assert_eq!(t.calculate_fee(1_000_000), 25_000);
    // 2.5% of 0 = 0
    assert_eq!(t.calculate_fee(0), 0);
}

#[test]
fn test_calculate_fee_custom_rate() {
    let t = ProtocolTreasury::new(100, admin()); // 1%
    assert_eq!(t.calculate_fee(10_000), 100);
}

#[test]
fn test_set_fee_rate() {
    let mut t = default_treasury();
    t.set_fee_rate(500, &admin()).unwrap();
    assert_eq!(t.get_fee_rate(), 500);
}

#[test]
fn test_set_fee_rate_too_high() {
    let mut t = default_treasury();
    let result = t.set_fee_rate(10_001, &admin());
    assert_eq!(result, Err(TreasuryError::InvalidFeeRate(10_001)));
}

#[test]
fn test_set_fee_rate_max_boundary() {
    let mut t = default_treasury();
    // Exactly 10_000 (100%) should be allowed
    assert!(t.set_fee_rate(10_000, &admin()).is_ok());
    assert_eq!(t.get_fee_rate(), 10_000);
}
