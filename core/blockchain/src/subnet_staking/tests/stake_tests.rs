use super::helpers::*;
use crate::subnet::SubnetId;
use crate::subnet_staking::types::SubnetStakingError;

#[test]
fn test_stake_success() {
    let mut m = manager();
    let p = provider(0x01);
    m.stake(p, SubnetId::Model, isa(1_000), isa(1_000), 10)
        .unwrap();

    assert!(m.is_staked(&p, &SubnetId::Model));
    let s = m.get_stake(&p, &SubnetId::Model).unwrap();
    assert_eq!(s.amount, isa(1_000));
    assert_eq!(s.staked_at, 10);
    assert!(s.unbonding.is_empty());
    assert_eq!(m.get_subnet_total_stake(&SubnetId::Model), isa(1_000));
}

#[test]
fn test_stake_below_minimum() {
    let mut m = manager();
    let p = provider(0x02);
    let err = m
        .stake(p, SubnetId::Tools, isa(499), isa(500), 1)
        .unwrap_err();
    assert_eq!(
        err,
        SubnetStakingError::InsufficientStake {
            required: isa(500),
            provided: isa(499),
        }
    );
    assert!(!m.is_staked(&p, &SubnetId::Tools));
}

#[test]
fn test_add_stake() {
    let mut m = manager();
    let p = provider(0x03);
    m.stake(p, SubnetId::Storage, isa(500), isa(500), 1)
        .unwrap();

    m.add_stake(&p, &SubnetId::Storage, isa(200)).unwrap();

    let s = m.get_stake(&p, &SubnetId::Storage).unwrap();
    assert_eq!(s.amount, isa(700));
    assert_eq!(m.get_subnet_total_stake(&SubnetId::Storage), isa(700));
}

#[test]
fn test_add_stake_no_stake_found() {
    let mut m = manager();
    let p = provider(0x04);
    let err = m.add_stake(&p, &SubnetId::Agent, isa(100)).unwrap_err();
    assert_eq!(err, SubnetStakingError::NoStakeFound);
}

#[test]
fn test_get_provider_total_stake() {
    let mut m = manager();
    let p = provider(0x0A);
    m.stake(p, SubnetId::Model, isa(1_000), isa(1_000), 1)
        .unwrap();
    m.stake(p, SubnetId::Tools, isa(500), isa(500), 1).unwrap();
    m.stake(p, SubnetId::Storage, isa(300), isa(300), 1)
        .unwrap();

    assert_eq!(m.get_provider_total_stake(&p), isa(1_800));
}

#[test]
fn test_get_subnet_total_stake() {
    let mut m = manager();
    let p1 = provider(0x0B);
    let p2 = provider(0x0C);
    m.stake(p1, SubnetId::Compute, isa(2_000), isa(2_000), 1)
        .unwrap();
    m.stake(p2, SubnetId::Compute, isa(3_000), isa(2_000), 1)
        .unwrap();

    assert_eq!(m.get_subnet_total_stake(&SubnetId::Compute), isa(5_000));
}

#[test]
fn test_provider_subnets() {
    let mut m = manager();
    let p = provider(0x0D);
    m.stake(p, SubnetId::Model, isa(1_000), isa(1_000), 1)
        .unwrap();
    m.stake(p, SubnetId::Agent, isa(1_000), isa(1_000), 1)
        .unwrap();

    let mut subnets = m.get_provider_subnets(&p);
    subnets.sort_by_key(|s| format!("{:?}", s));
    assert_eq!(subnets.len(), 2);
    assert!(subnets.contains(&SubnetId::Model));
    assert!(subnets.contains(&SubnetId::Agent));
}

#[test]
fn test_no_stake_found() {
    let mut m = manager();
    let p = provider(0xFF);

    assert_eq!(
        m.add_stake(&p, &SubnetId::Tools, isa(100)),
        Err(SubnetStakingError::NoStakeFound)
    );
    assert_eq!(
        m.begin_unstake(&p, &SubnetId::Tools, isa(100), 1),
        Err(SubnetStakingError::NoStakeFound)
    );
    assert_eq!(
        m.slash(&p, &SubnetId::Tools, 1_000),
        Err(SubnetStakingError::NoStakeFound)
    );
    assert_eq!(m.complete_unbonding(&p, &SubnetId::Tools, 1000), 0);
}

#[test]
fn test_already_staked() {
    let mut m = manager();
    let p = provider(0x0E);
    m.stake(p, SubnetId::Market, isa(500), isa(500), 1)
        .unwrap();

    let err = m
        .stake(p, SubnetId::Market, isa(500), isa(500), 2)
        .unwrap_err();
    assert_eq!(err, SubnetStakingError::AlreadyStaked);
}
