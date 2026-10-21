use super::helpers::*;
use crate::subnet::SubnetId;
use crate::subnet_staking::types::SubnetStakingError;

#[test]
fn test_begin_unstake() {
    let mut m = manager();
    let p = provider(0x05);
    m.stake(p, SubnetId::Compute, isa(2_000), isa(2_000), 1)
        .unwrap();

    m.begin_unstake(&p, &SubnetId::Compute, isa(500), 50)
        .unwrap();

    let s = m.get_stake(&p, &SubnetId::Compute).unwrap();
    assert_eq!(s.amount, isa(1_500));
    assert_eq!(s.unbonding.len(), 1);
    assert_eq!(s.unbonding[0].amount, isa(500));
    assert_eq!(s.unbonding[0].completion_height, 150); // 50 + 100
    assert_eq!(m.get_subnet_total_stake(&SubnetId::Compute), isa(1_500));
}

#[test]
fn test_begin_unstake_insufficient_balance() {
    let mut m = manager();
    let p = provider(0x06);
    m.stake(p, SubnetId::Model, isa(1_000), isa(1_000), 1)
        .unwrap();

    let err = m
        .begin_unstake(&p, &SubnetId::Model, isa(1_001), 10)
        .unwrap_err();
    assert_eq!(err, SubnetStakingError::InsufficientBalance);
    // Stake unchanged
    assert_eq!(
        m.get_stake(&p, &SubnetId::Model).unwrap().amount,
        isa(1_000)
    );
}

#[test]
fn test_complete_unbonding() {
    let mut m = manager();
    let p = provider(0x07);
    m.stake(p, SubnetId::Tools, isa(500), isa(500), 1).unwrap();
    m.begin_unstake(&p, &SubnetId::Tools, isa(200), 1).unwrap();

    // Before period ends — nothing released.
    let early = m.complete_unbonding(&p, &SubnetId::Tools, 50);
    assert_eq!(early, 0);
    assert_eq!(
        m.get_stake(&p, &SubnetId::Tools).unwrap().unbonding.len(),
        1
    );

    // At completion height — full amount released.
    let released = m.complete_unbonding(&p, &SubnetId::Tools, 101); // 1 + 100
    assert_eq!(released, isa(200));
    assert!(
        m.get_stake(&p, &SubnetId::Tools)
            .unwrap()
            .unbonding
            .is_empty()
    );
}

#[test]
fn test_slash() {
    let mut m = manager();
    let p = provider(0x08);
    m.stake(p, SubnetId::Market, isa(1_000), isa(500), 1)
        .unwrap();

    // Slash 10% (1000 bps)
    let slashed = m.slash(&p, &SubnetId::Market, 1_000).unwrap();
    assert_eq!(slashed, isa(100));
    assert_eq!(
        m.get_stake(&p, &SubnetId::Market).unwrap().amount,
        isa(900)
    );
    assert_eq!(m.get_subnet_total_stake(&SubnetId::Market), isa(900));
}

#[test]
fn test_slash_max_cap() {
    let mut m = manager();
    let p = provider(0x09);
    m.stake(p, SubnetId::Agent, isa(1_000), isa(1_000), 1)
        .unwrap();

    // Request 80% (8000 bps) — capped at 50% (5000 bps).
    let slashed = m.slash(&p, &SubnetId::Agent, 8_000).unwrap();
    assert_eq!(slashed, isa(500));
    assert_eq!(
        m.get_stake(&p, &SubnetId::Agent).unwrap().amount,
        isa(500)
    );
}
