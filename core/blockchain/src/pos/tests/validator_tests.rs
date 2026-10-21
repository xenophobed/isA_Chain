use super::helpers::*;
use crate::pos::types::PoSError;
use crate::types::constants::VALIDATOR_MIN_STAKE;

#[test]
fn test_register_validator() {
    let mut e = engine();
    let a = addr(0x01);
    e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();

    let validators = e.get_validators();
    assert_eq!(validators.len(), 1);
    assert_eq!(validators[0].address, a);
    assert_eq!(validators[0].stake, VALIDATOR_MIN_STAKE);
    assert_eq!(validators[0].weight, 1);
    assert!(e.staking_vault.is_staked(&a));
}

#[test]
fn test_register_duplicate_fails() {
    let mut e = engine();
    let a = addr(0x02);
    e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();
    let result = e.register_validator(a, VALIDATOR_MIN_STAKE, 2);
    assert_eq!(result, Err(PoSError::DuplicateValidator(a)));
    // Only one entry should exist
    assert_eq!(e.get_validators().len(), 1);
}

#[test]
fn test_remove_validator() {
    let mut e = engine();
    let a = addr(0x03);
    let b = addr(0x04);
    e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();
    e.register_validator(b, VALIDATOR_MIN_STAKE, 1).unwrap();

    e.remove_validator(&a).unwrap();
    assert_eq!(e.get_validators().len(), 1);
    assert!(e.get_validator(&a).is_none());
    assert!(e.get_validator(&b).is_some());
}

#[test]
fn test_remove_nonexistent_validator_fails() {
    let mut e = engine();
    let a = addr(0xAA);
    assert_eq!(e.remove_validator(&a), Err(PoSError::ValidatorNotFound(a)));
}

#[test]
fn test_record_proposed_and_missed() {
    let mut e = engine();
    let a = addr(0x0F);
    e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();

    e.record_block_proposed(&a).unwrap();
    e.record_block_proposed(&a).unwrap();
    e.record_block_missed(&a).unwrap();

    let v = e.get_validator(&a).unwrap();
    assert_eq!(v.blocks_proposed, 2);
    assert_eq!(v.blocks_missed, 1);
}

#[test]
fn test_slash_validator() {
    let mut e = engine();
    let a = addr(0x10);
    e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();

    // Slash 10% (1000 bps)
    let slashed = e.slash_validator(&a, 1000).unwrap();
    let expected = VALIDATOR_MIN_STAKE / 10;
    assert_eq!(slashed, expected);

    let v = e.get_validator(&a).unwrap();
    assert_eq!(v.stake, VALIDATOR_MIN_STAKE - expected);
    // weight should be updated
    assert_eq!(v.weight, crate::pos::types::stake_to_weight(VALIDATOR_MIN_STAKE - expected));
}
