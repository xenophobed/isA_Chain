use super::helpers::*;
use crate::pos::engine::PoSConsensus;
use crate::pos::types::PoSError;
use crate::types::constants::VALIDATOR_MIN_STAKE;

#[test]
fn test_select_proposer_deterministic() {
    let mut e = engine();
    let a = addr(0x05);
    e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();

    // Same height always returns the same address
    let p1 = e.select_proposer(42).unwrap();
    let p2 = e.select_proposer(42).unwrap();
    assert_eq!(p1, p2);
}

#[test]
fn test_select_proposer_weighted() {
    let mut e = engine();
    // Validator A: weight 1 (1× min stake)
    // Validator B: weight 3 (3× min stake)
    // Total weight = 4; A wins for slot 0, B wins for slots 1, 2, 3
    let a = addr(0x0A);
    let b = addr(0x0B);
    e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();
    e.register_validator(b, VALIDATOR_MIN_STAKE * 3, 1).unwrap();

    assert_eq!(e.total_voting_power(), 4);

    // Slot 0 → A (cumulative weight after A = 1 > 0)
    let proposer_0 = e.select_proposer(0).unwrap(); // slot = 0 % 4 = 0
    assert_eq!(proposer_0, a);

    // Slots 1, 2, 3 → B
    let proposer_1 = e.select_proposer(1).unwrap(); // slot = 1
    let proposer_2 = e.select_proposer(2).unwrap(); // slot = 2
    let proposer_3 = e.select_proposer(3).unwrap(); // slot = 3
    assert_eq!(proposer_1, b);
    assert_eq!(proposer_2, b);
    assert_eq!(proposer_3, b);
}

#[test]
fn test_validate_block() {
    let mut e = engine();
    let a = addr(0x0C);
    e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();

    // The single validator should always be the valid proposer
    assert!(e.validate_block(&a, 10).is_ok());
}

#[test]
fn test_validate_wrong_proposer() {
    let mut e = engine();
    let a = addr(0x0D);
    let wrong = addr(0xFF);
    e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();

    let result = e.validate_block(&wrong, 10);
    assert_eq!(result, Err(PoSError::InvalidProposer(wrong)));
}

#[test]
fn test_insufficient_validators() {
    // Engine that requires at least 2 validators
    let mut e = PoSConsensus::new(100, 2);
    let a = addr(0x0E);
    e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();

    let result = e.select_proposer(1);
    assert_eq!(result, Err(PoSError::InsufficientValidators(2)));
}

#[test]
fn test_epoch_boundary() {
    let e = PoSConsensus::new(100, 1);

    // Block 0 is NOT an epoch boundary
    assert!(!e.is_epoch_boundary(0));
    // Not multiples of epoch_length
    assert!(!e.is_epoch_boundary(1));
    assert!(!e.is_epoch_boundary(99));
    // Exact multiples are epoch boundaries
    assert!(e.is_epoch_boundary(100));
    assert!(e.is_epoch_boundary(200));
    assert!(e.is_epoch_boundary(500));
}

#[test]
fn test_total_voting_power() {
    let mut e = engine();
    assert_eq!(e.total_voting_power(), 0);

    let a = addr(0x20);
    let b = addr(0x21);
    e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();
    assert_eq!(e.total_voting_power(), 1);

    e.register_validator(b, VALIDATOR_MIN_STAKE * 4, 1).unwrap();
    assert_eq!(e.total_voting_power(), 5);

    e.remove_validator(&a).unwrap();
    assert_eq!(e.total_voting_power(), 4);
}
