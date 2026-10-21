use super::*;
use crate::types::Address;

#[test]
fn test_account_creation() {
    let account = Account::new_external(1000);

    assert_eq!(account.balance, 1000);
    assert_eq!(account.nonce, 0);
    assert!(!account.is_contract());
    assert!(!account.is_validator());
}

#[test]
fn test_balance_operations() {
    let mut account = Account::new_external(1000);

    // Test successful deduction
    assert!(account.deduct_balance(500).is_ok());
    assert_eq!(account.balance, 500);

    // Test insufficient balance
    assert!(account.deduct_balance(600).is_err());

    // Test adding balance
    account.add_balance(200);
    assert_eq!(account.balance, 700);
}

#[test]
fn test_validator_creation() {
    let mut account = Account::new_external(50000);

    let description = ValidatorDescription {
        moniker: "test-validator".to_string(),
        identity: "".to_string(),
        website: "".to_string(),
        security_contact: "".to_string(),
        details: "".to_string(),
    };

    assert!(account
        .become_validator(
            vec![1u8; 33],
            1000, // 10% commission
            32000,
            32000,
            description,
            0
        )
        .is_ok());

    assert!(account.is_validator());
    assert!(account.is_active_validator());
}

#[test]
fn test_delegation() {
    let mut account = Account::new_external(1000);
    let validator = Address::from([1u8; 20]);

    assert!(account.delegate_to_validator(validator, 500, 0).is_ok());

    let staking_info = account.staking_info.as_ref().unwrap();
    assert_eq!(staking_info.total_staked, 500);
    assert!(staking_info.delegations.contains_key(&validator));
}
