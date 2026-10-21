use super::helpers::*;
use crate::emission::types::EmissionError;
use crate::subnet::SubnetId;
use crate::types::{Address, Amount};
use std::collections::HashMap;

#[test]
fn test_emit_distribution() {
    let mut c = controller();
    let weights = two_subnet_weights();
    let emissions = c.emit(1, &weights, &empty_stakes()).unwrap();

    assert_eq!(emissions.len(), 2);

    let total: Amount = emissions.iter().map(|e| e.amount).sum();
    // Total should equal block_reward (100 ISA). Small rounding loss acceptable.
    assert!(total <= 100 * ISA);
    assert!(total >= 99 * ISA, "total {total} should be close to 100 ISA");
}

#[test]
fn test_emit_proportional_by_weight() {
    let mut c = controller();
    let weights = two_subnet_weights(); // Model=6000, Tools=4000
    let emissions = c.emit(1, &weights, &empty_stakes()).unwrap();

    let model_amt = emissions
        .iter()
        .find(|e| e.subnet_id == SubnetId::Model)
        .unwrap()
        .amount;
    let tools_amt = emissions
        .iter()
        .find(|e| e.subnet_id == SubnetId::Tools)
        .unwrap()
        .amount;

    // Model should get 60 ISA, Tools 40 ISA.
    assert_eq!(model_amt, 60 * ISA);
    assert_eq!(tools_amt, 40 * ISA);
}

#[test]
fn test_emit_proportional_by_stake() {
    let mut c = controller();

    // Single subnet: Model = 10 000 weight (100%).
    let mut weights = HashMap::new();
    weights.insert(SubnetId::Model, 10_000u32);

    // Two providers: p1 stakes 3x, p2 stakes 1x → 75% / 25%.
    let p1 = addr(0x01);
    let p2 = addr(0x02);
    let mut stakes: HashMap<(SubnetId, Address), Amount> = HashMap::new();
    stakes.insert((SubnetId::Model, p1), 300 * ISA);
    stakes.insert((SubnetId::Model, p2), 100 * ISA);

    let emissions = c.emit(1, &weights, &stakes).unwrap();
    assert_eq!(emissions.len(), 1);

    let e = &emissions[0];
    assert_eq!(e.subnet_id, SubnetId::Model);

    let shares: HashMap<Address, Amount> = e.provider_shares.iter().cloned().collect();
    let s1 = *shares.get(&p1).unwrap();
    let s2 = *shares.get(&p2).unwrap();

    // s1 should be 75 ISA, s2 should be 25 ISA.
    assert_eq!(s1, 75 * ISA);
    assert_eq!(s2, 25 * ISA);
}

#[test]
fn test_subnet_totals() {
    let mut c = controller();
    let weights = two_subnet_weights(); // Model 60%, Tools 40%

    c.emit(1, &weights, &empty_stakes()).unwrap();
    c.emit(2, &weights, &empty_stakes()).unwrap();

    // After 2 blocks: Model = 2 * 60 = 120 ISA, Tools = 2 * 40 = 80 ISA.
    assert_eq!(c.get_subnet_total(&SubnetId::Model), 120 * ISA);
    assert_eq!(c.get_subnet_total(&SubnetId::Tools), 80 * ISA);
    assert_eq!(c.get_total_emitted(), 200 * ISA);
}

#[test]
fn test_already_emitted() {
    let mut c = controller();
    let weights = two_subnet_weights();

    c.emit(5, &weights, &empty_stakes()).unwrap();
    let err = c.emit(5, &weights, &empty_stakes()).unwrap_err();
    assert_eq!(err, EmissionError::AlreadyEmitted(5));
}

#[test]
fn test_no_subnets_fails() {
    let mut c = controller();
    let err = c.emit(1, &HashMap::new(), &empty_stakes()).unwrap_err();
    assert_eq!(err, EmissionError::NoSubnets);
}

#[test]
fn test_emission_history() {
    let mut c = controller();
    let weights = two_subnet_weights();

    c.emit(10, &weights, &empty_stakes()).unwrap();

    let records = c.get_emission_at(10).unwrap();
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|e| e.height == 10));

    // Query for a block that was never emitted returns None.
    assert!(c.get_emission_at(99).is_none());
}

#[test]
fn test_multiple_emissions() {
    let mut c = controller();
    let weights = two_subnet_weights();

    for height in 1..=5u64 {
        let emissions = c.emit(height, &weights, &empty_stakes()).unwrap();
        assert_eq!(emissions.len(), 2);
    }

    // 5 blocks × 100 ISA each = 500 ISA total.
    assert_eq!(c.get_total_emitted(), 500 * ISA);
    // history should contain 5 × 2 = 10 records.
    assert_eq!(c.emission_history.len(), 10);
}

#[test]
fn test_invalid_weights_zero_sum() {
    let mut c = controller();
    let mut weights = HashMap::new();
    weights.insert(SubnetId::Model, 0u32);
    let err = c.emit(1, &weights, &empty_stakes()).unwrap_err();
    assert_eq!(err, EmissionError::InvalidWeights);
}
